use std::{fs, path::PathBuf, process::Command};

use assert_cmd::prelude::*;
use tempfile::tempdir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask has a parent")
        .to_path_buf()
}

#[test]
fn dashboard_parity_cli_respects_cli_overrides() -> Result<(), Box<dyn std::error::Error>> {
    let root = repo_root();
    let tmp = tempdir()?;

    let dashboard = r#"
{
  "panels": [
    {
      "title": "submission_latency_p95",
      "type": "timeseries",
      "fieldConfig": {
        "defaults": {
          "unit": "ms",
          "custom": {
            "metric_id": "submission_latency_p95"
          }
        }
      },
      "targets": [
        {
          "expr": "histogram_quantile(0.95, sum(rate(android_submission_latency_bucket[5m])))"
        }
      ]
    }
  ]
}
"#;

    let android_dash = tmp.path().join("android.json");
    let rust_dash = tmp.path().join("rust.json");
    fs::write(&android_dash, dashboard)?;
    fs::write(&rust_dash, dashboard)?;

    let allowances = tmp.path().join("allowances.json");
    fs::write(
        &allowances,
        r#"{
  "missing_in_android": [],
  "missing_in_rust": [],
  "unit_diff": [],
  "target_diff": [],
  "expr_diff": []
}
"#,
    )?;

    let artifact = tmp.path().join("diff.json");
    Command::new("python3")
        .current_dir(&root)
        .arg("scripts/telemetry/compare_dashboards.py")
        .arg("--android")
        .arg(&android_dash)
        .arg("--rust")
        .arg(&rust_dash)
        .arg("--allow-file")
        .arg(&allowances)
        .arg("--output")
        .arg(&artifact)
        .assert()
        .success();

    Command::new("ci/check_android_dashboard_parity.sh")
        .current_dir(&root)
        .arg("--android")
        .arg(&android_dash)
        .arg("--rust")
        .arg(&rust_dash)
        .arg("--allowance")
        .arg(&allowances)
        .arg("--artifact")
        .arg(&artifact)
        .assert()
        .success();

    Ok(())
}
