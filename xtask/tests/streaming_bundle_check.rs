use std::{fs, path::PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use norito::{
    json::{self as serde_json, Value},
    streaming::BUNDLED_RANS_BUILD_AVAILABLE,
};
use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn run_bundle_check(config_relative: &str) -> Value {
    let temp = TempDir::new().expect("temp dir");
    let json_out = temp.path().join("summary.json");
    let config_path = workspace_root().join(config_relative);

    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.args([
        "streaming-bundle-check",
        "--config",
        config_path.to_str().expect("utf8 config path"),
        "--json-out",
        json_out.to_str().expect("utf8 json path"),
    ]);
    cmd.assert().success();

    let raw = fs::read_to_string(json_out).expect("bundle summary JSON");
    serde_json::from_str(&raw).expect("parse bundle summary JSON")
}

fn assert_tables_path(summary: &Value) {
    let tables = summary["tables"]
        .as_object()
        .expect("tables block should be an object");
    let path = tables["path"]
        .as_str()
        .expect("tables.path should be a string");
    assert!(
        path.ends_with("codec/rans/tables/rans_seed0.toml"),
        "tables path `{path}` should point at the default deterministic tables"
    );
    let checksum = tables["checksum"]
        .as_str()
        .expect("tables checksum should be a string");
    assert_eq!(
        checksum.len(),
        64,
        "tables checksum should be a 32-byte hex digest"
    );
}

#[test]
fn streaming_bundle_check_reports_bundled_requirements() {
    if !BUNDLED_RANS_BUILD_AVAILABLE {
        eprintln!("skipping bundled entropy test (ENABLE_RANS_BUNDLES not enabled)");
        return;
    }

    let summary = run_bundle_check("crates/iroha_config/tests/fixtures/streaming_bundled.toml");
    assert!(
        summary["bundle_required"]
            .as_bool()
            .expect("bundle_required should be boolean"),
        "bundled config should mark bundle_required=true"
    );
    assert_eq!(
        summary["entropy_mode"].as_str(),
        Some("rans_bundled"),
        "bundled config should report bundled entropy mode"
    );
    assert_eq!(
        summary["bundle_accel"].as_str(),
        Some("cpu_simd"),
        "bundled config should surface CPU SIMD acceleration"
    );
    assert_eq!(
        summary["bundle_width"].as_i64(),
        Some(3),
        "bundled config should preserve the configured bundle width"
    );
    assert_eq!(
        summary["gpu_build_available"].as_bool(),
        Some(norito::streaming::BUNDLED_RANS_GPU_BUILD_AVAILABLE),
        "gpu_build_available should mirror the compiled Norito features"
    );
    assert_eq!(
        summary["bundle_accel_allowed"].as_bool(),
        Some(true),
        "cpu_simd acceleration should always be allowed when bundled mode is enabled"
    );
    assert_tables_path(&summary);
}
