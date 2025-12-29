use std::{fs, path::Path};

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};
use tempfile::tempdir;

#[test]
fn soranet_gateway_billing_pack_is_deterministic() {
    let temp = tempdir().expect("tempdir");
    let out_dir = temp.path().join("billing");

    let mut cmd = cargo_bin_cmd!("xtask");
    let output = cmd
        .args([
            "soranet-gateway-billing-m0",
            "--output-dir",
            out_dir.to_str().expect("utf8"),
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-gateway-billing-m0");
    assert!(
        output.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let baseline_dir = repo_root
        .join("configs")
        .join("soranet")
        .join("gateway_m0")
        .join("billing");

    let catalog =
        fs::read_to_string(out_dir.join("billing_meter_catalog.json")).expect("catalog exists");
    let baseline_catalog =
        fs::read_to_string(baseline_dir.join("billing_meter_catalog.json")).expect("baseline");
    assert_eq!(catalog, baseline_catalog, "meter catalog drifted");

    let rating_plan =
        fs::read_to_string(out_dir.join("billing_rating_plan.toml")).expect("rating plan exists");
    let baseline_rating =
        fs::read_to_string(baseline_dir.join("billing_rating_plan.toml")).expect("baseline plan");
    assert_eq!(rating_plan, baseline_rating, "rating plan drifted");

    let ledger_hooks =
        fs::read_to_string(out_dir.join("billing_ledger_hooks.toml")).expect("hooks exist");
    let baseline_hooks =
        fs::read_to_string(baseline_dir.join("billing_ledger_hooks.toml")).expect("baseline");
    assert_eq!(ledger_hooks, baseline_hooks, "ledger hooks drifted");

    let guardrails =
        fs::read_to_string(out_dir.join("billing_guardrails.yaml")).expect("guardrails exist");
    let baseline_guardrails =
        fs::read_to_string(baseline_dir.join("billing_guardrails.yaml")).expect("baseline");
    assert_eq!(guardrails, baseline_guardrails, "guardrails drifted");

    let summary_bytes = fs::read(out_dir.join("billing_m0_summary.json")).expect("summary exists");
    let summary: Value = json::from_slice(&summary_bytes).expect("summary parses");
    assert_eq!(
        summary["billing_period"],
        Value::from("2026-11"),
        "summary billing period mismatch"
    );
    assert!(
        summary["meter_catalog_path"]
            .as_str()
            .unwrap_or_default()
            .ends_with("billing_meter_catalog.json"),
        "summary catalog path not recorded"
    );
    assert!(
        summary["invoice_template_path"]
            .as_str()
            .unwrap_or_default()
            .ends_with("billing_invoice_template.md"),
        "summary invoice path not recorded"
    );
}
