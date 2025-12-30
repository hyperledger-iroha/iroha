#![cfg(feature = "cli")]

use std::{collections::HashMap, fs, path::PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};
use tempfile::tempdir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate dir has parent")
        .parent()
        .expect("workspace has root")
        .to_path_buf()
}

fn example_dir() -> PathBuf {
    repo_root().join("docs/examples/sorafs_capacity_simulation")
}

fn run_cli(args: &[String]) -> assert_cmd::assert::Assert {
    let mut cmd = cargo_bin_cmd!("sorafs_manifest_stub");
    cmd.arg("capacity");
    for arg in args {
        cmd.arg(arg);
    }
    cmd.assert()
}

#[test]
fn quota_negotiation_fixtures_are_consistent() {
    let temp = tempdir().expect("tempdir");
    let base = example_dir().join("scenarios/quota_negotiation");

    let declarations = [("alpha", 480_u64), ("beta", 320_u64), ("gamma", 240_u64)];
    let mut committed: HashMap<String, u64> = HashMap::new();

    for (alias, expected_committed) in declarations {
        let spec = base.join(format!("provider_{alias}_declaration.json"));
        let json_out = temp
            .path()
            .join(format!("provider_{alias}_declaration_summary.json"));
        run_cli(&[
            "declaration".to_owned(),
            format!("--spec={}", spec.display()),
            format!("--json-out={}", json_out.display()),
            "--quiet".to_owned(),
        ])
        .success();

        let summary_bytes = fs::read(&json_out).expect("read summary");
        let summary: Value = json::from_slice(&summary_bytes).expect("summary json parses");
        let committed_gib = summary
            .get("committed_capacity_gib")
            .and_then(Value::as_u64)
            .expect("committed_capacity_gib present");
        assert_eq!(
            committed_gib, expected_committed,
            "{alias} committed capacity mismatch"
        );
        let provider_id = summary
            .get("provider_id_hex")
            .and_then(Value::as_str)
            .expect("provider_id present")
            .to_string();
        committed.insert(provider_id, committed_gib);
    }

    let repl_spec = base.join("replication_order.json");
    let repl_summary = temp.path().join("replication_order_summary.json");
    run_cli(&[
        "replication-order".to_owned(),
        format!("--spec={}", repl_spec.display()),
        format!("--json-out={}", repl_summary.display()),
        "--quiet".to_owned(),
    ])
    .success();

    let repl_bytes = fs::read(&repl_summary).expect("read replication summary");
    let repl_value: Value = json::from_slice(&repl_bytes).expect("parse replication summary");

    let assignments = repl_value
        .get("assignments")
        .and_then(Value::as_array)
        .expect("assignments array");
    let mut total_assigned = 0_u64;
    for entry in assignments {
        let provider_id = entry
            .get("provider_id_hex")
            .and_then(Value::as_str)
            .expect("provider id");
        let slice_gib = entry
            .get("slice_gib")
            .and_then(Value::as_u64)
            .expect("slice gib");
        total_assigned += slice_gib;
        if let Some(committed_gib) = committed.get(provider_id) {
            assert!(
                slice_gib <= *committed_gib,
                "assignment {slice_gib} exceeds committed capacity {committed_gib} for {provider_id}"
            );
        } else {
            panic!("replication assignment references unknown provider {provider_id}");
        }
    }

    let target = repl_value
        .get("assignments")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter().fold(0_u64, |acc, entry| {
                acc + entry.get("slice_gib").and_then(Value::as_u64).unwrap_or(0)
            })
        })
        .expect("total assignment sum");

    assert_eq!(
        total_assigned, target,
        "sum of assigned GiB should match computed total"
    );
}

#[test]
fn failover_and_slashing_fixtures_validate() {
    let temp = tempdir().expect("tempdir");
    let failover_base = example_dir().join("scenarios/failover");

    let telemetry_cases = [
        ("alpha_primary", 0_u64, 99800_u64),
        ("alpha_outage", 7_u64, 72000_u64),
        ("beta_failover", 1_u64, 99500_u64),
    ];

    let mut summaries = HashMap::new();

    for (alias, expected_failed, expected_uptime) in telemetry_cases {
        let spec = failover_base.join(format!("telemetry_{alias}.json"));
        let json_out = temp.path().join(format!("telemetry_{alias}_summary.json"));
        run_cli(&[
            "telemetry".to_owned(),
            format!("--spec={}", spec.display()),
            format!("--json-out={}", json_out.display()),
            "--quiet".to_owned(),
        ])
        .success();

        let summary_bytes = fs::read(&json_out).expect("read telemetry summary");
        let summary: Value = json::from_slice(&summary_bytes).expect("telemetry summary json");
        let failed = summary
            .get("failed_replications")
            .and_then(Value::as_u64)
            .expect("failed_replications");
        let uptime = summary
            .get("uptime_percent_milli")
            .and_then(Value::as_u64)
            .expect("uptime");
        assert_eq!(
            failed, expected_failed,
            "{alias} failed replication mismatch"
        );
        assert_eq!(uptime, expected_uptime, "{alias} uptime mismatch");
        summaries.insert(alias.to_owned(), summary);
    }

    let primary_uptime = summaries["alpha_primary"]
        .get("uptime_percent_milli")
        .and_then(Value::as_u64)
        .unwrap();
    let outage_uptime = summaries["alpha_outage"]
        .get("uptime_percent_milli")
        .and_then(Value::as_u64)
        .unwrap();
    assert!(
        outage_uptime < primary_uptime,
        "alpha outage uptime should drop below primary snapshot"
    );

    // Slashing fixture
    let slashing_spec = example_dir().join("scenarios/slashing/capacity_dispute.json");
    let slashing_summary = temp.path().join("capacity_dispute_summary.json");
    run_cli(&[
        "dispute".to_owned(),
        format!("--spec={}", slashing_spec.display()),
        format!("--json-out={}", slashing_summary.display()),
        "--quiet".to_owned(),
    ])
    .success();

    let slashing_bytes = fs::read(&slashing_summary).expect("read slashing summary");
    let slashing: Value = json::from_slice(&slashing_bytes).expect("slashing summary json");
    assert_eq!(
        slashing.get("kind").and_then(Value::as_str),
        Some("replication_shortfall"),
        "expected dispute kind to match fixture"
    );
    assert!(
        slashing
            .get("requested_remedy")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("Slash"),
        "remedy should request a slashing action"
    );
}
