use std::{fs, path::PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};
use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn da_proof_bench_emits_reports() {
    let temp = TempDir::new().expect("temp dir");
    let json_out = temp.path().join("proof_bench.json");
    let markdown_out = temp.path().join("proof_bench.md");
    let payload_out = temp.path().join("payload.bin");

    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.args([
        "da-proof-bench",
        "--iterations",
        "2",
        "--sample-count",
        "2",
        "--budget-ms",
        "1000",
        "--payload",
        payload_out.to_str().expect("utf8 path"),
        "--payload-bytes",
        "262144",
        "--json-out",
        json_out.to_str().expect("utf8 path"),
        "--markdown-out",
        markdown_out.to_str().expect("utf8 path"),
    ]);
    cmd.assert().success();

    let json_text = fs::read_to_string(&json_out).expect("json output");
    let value: Value = json::from_str(&json_text).expect("parse json");
    assert!(value.get("runs").is_some(), "runs missing in report");
    assert!(
        value
            .get("stats")
            .and_then(Value::as_object)
            .and_then(|map| map.get("recommended_budget_ms"))
            .is_some(),
        "stats missing in report"
    );
    assert!(markdown_out.exists(), "markdown output missing");
    assert!(payload_out.exists(), "generated payload missing");
}
