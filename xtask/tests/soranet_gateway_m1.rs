use std::{fs, path::Path};

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};
use tempfile::tempdir;

#[test]
fn soranet_gateway_m1_bundle_is_emitted() {
    let temp = tempdir().expect("tempdir");
    let out_dir = temp.path().join("gateway_m1");
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let config = workspace_root
        .join("configs")
        .join("soranet")
        .join("gateway_m1")
        .join("alpha_config.json");

    let mut cmd = cargo_bin_cmd!("xtask");
    let output = cmd
        .current_dir(workspace_root)
        .args([
            "soranet-gateway-m1",
            "--config",
            config.to_str().expect("utf8"),
            "--output-dir",
            out_dir.to_str().expect("utf8"),
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-gateway-m1");
    assert!(
        output.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let summary_path = out_dir.join("gateway_m1_summary.json");
    let summary_bytes = fs::read(&summary_path).expect("summary JSON exists");
    let summary: Value = json::from_slice(&summary_bytes).expect("summary parses");

    let pops = summary["pops"].as_array().expect("pops array");
    assert_eq!(pops.len(), 3, "alpha bundle should include three pops");
    for pop in pops {
        let name = pop["name"].as_str().expect("pop name");
        let bundle = pop["bundle_manifest"]
            .as_str()
            .expect("bundle manifest path");
        let gateway = pop["gateway_summary"]
            .as_str()
            .expect("gateway summary path");
        let ops = pop["ops_summary"].as_str().expect("ops summary path");
        assert!(
            out_dir.join(bundle).is_file(),
            "bundle manifest missing for {name}"
        );
        assert!(
            out_dir.join(gateway).is_file(),
            "gateway summary missing for {name}"
        );
        assert!(
            out_dir.join(ops).is_file(),
            "ops summary missing for {name}"
        );
    }

    let billing_total = summary["billing"]["totals_micros"]
        .as_u64()
        .expect("billing totals_micros");
    assert_eq!(
        billing_total, 124_203_000,
        "billing totals should track the sample usage"
    );
    let invoice_path = summary["billing"]["invoice"]
        .as_str()
        .expect("invoice path");
    assert!(
        out_dir.join(invoice_path).is_file(),
        "billing invoice missing"
    );

    let fed_summary = summary["federated_ops"]["summary"]
        .as_str()
        .expect("federated summary path");
    let rotation_md = summary["federated_ops"]["gameday_rotation_markdown"]
        .as_str()
        .expect("rotation markdown path");
    assert!(
        out_dir.join(fed_summary).is_file(),
        "federated ops summary missing"
    );
    assert!(
        out_dir.join(rotation_md).is_file(),
        "rotation markdown missing"
    );
}
