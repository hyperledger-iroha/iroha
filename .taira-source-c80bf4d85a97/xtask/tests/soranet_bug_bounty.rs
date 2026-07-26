use std::{fs, path::Path};

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};
use tempfile::tempdir;

#[test]
fn bug_bounty_pack_is_emitted() {
    let temp = tempdir().expect("tempdir");
    let out_dir = temp.path().join("bounty");
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate resides in workspace root");
    let fixture = workspace_root
        .join("fixtures")
        .join("soranet_bug_bounty")
        .join("sample_plan.json");

    let mut cmd = cargo_bin_cmd!("xtask");
    let output = cmd
        .args([
            "soranet-bug-bounty",
            "--config",
            fixture.to_str().expect("fixture utf8"),
            "--output-dir",
            out_dir.to_str().expect("utf8"),
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-bug-bounty");
    assert!(
        output.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    for path in [
        out_dir.join("bug_bounty_summary.json"),
        out_dir.join("bug_bounty_overview.md"),
        out_dir.join("triage_checklist.md"),
        out_dir.join("remediation_template.md"),
    ] {
        assert!(
            path.exists(),
            "missing generated artifact: {}",
            path.display()
        );
    }

    let summary_bytes = fs::read(out_dir.join("bug_bounty_summary.json")).expect("summary exists");
    let summary: Value = json::from_slice(&summary_bytes).expect("parse summary");
    assert_eq!(
        summary["slug"],
        norito::json!("snnet-15h1-kit"),
        "slug mismatch"
    );
    assert_eq!(
        summary["scope"].as_array().map(|entries| entries.len()),
        Some(3),
        "scope expected 3 entries"
    );
    assert!(
        summary["outputs"]["triage_checklist"]
            .as_str()
            .expect("triage path")
            .contains("triage_checklist.md"),
        "triage path missing"
    );
}
