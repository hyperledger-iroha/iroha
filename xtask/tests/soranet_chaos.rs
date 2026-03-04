use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};
use tempfile::tempdir;

#[test]
fn soranet_chaos_kit_and_report_round_trip() {
    let temp = tempdir().expect("tempdir");
    let kit_dir = temp.path().join("kit");
    let log_path = kit_dir.join("chaos_events.ndjson");
    let summary_path = temp.path().join("summary.json");

    let mut kit_cmd = cargo_bin_cmd!("xtask");
    let kit_output = kit_cmd
        .args([
            "soranet-chaos-kit",
            "--out",
            kit_dir.to_str().expect("utf8"),
            "--pop",
            "pop-a",
            "--gateway",
            "gw-a",
            "--resolver",
            "resolver-a",
            "--quarter",
            "2027-Q2",
            "--now",
            "1700000000",
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-chaos-kit");
    assert!(
        kit_output.status.success(),
        "chaos kit command failed: {kit_output:?}"
    );

    let plan_json = kit_dir.join("plan.json");
    let plan_md = kit_dir.join("plan.md");
    assert!(plan_json.exists(), "plan.json missing");
    assert!(plan_md.exists(), "plan.md missing");

    let plan: Value = json::from_reader(fs::File::open(&plan_json).expect("open plan.json"))
        .expect("decode plan");
    assert_eq!(
        Some("2027-Q2"),
        plan.get("quarter").and_then(Value::as_str),
        "quarter label must match"
    );
    let Some(Value::Object(targets)) = plan.get("targets") else {
        panic!("targets missing");
    };
    assert_eq!(
        Some("pop-a"),
        targets.get("pop").and_then(Value::as_str),
        "pop label must match"
    );
    assert!(kit_dir.join("scripts/prefix_withdrawal.sh").exists());
    assert!(
        kit_dir
            .join("scripts/trustless_verifier_failure.sh")
            .exists()
    );
    assert!(kit_dir.join("scripts/resolver_brownout.sh").exists());
    assert!(kit_dir.join("scripts/log_event.sh").exists());
    assert!(log_path.exists(), "log file stub missing");

    let log_entries = [
        r#"{"ts_ms":1000,"scenario":"prefix-withdrawal","action":"inject"}"#,
        r#"{"ts_ms":1300,"scenario":"prefix-withdrawal","action":"detect"}"#,
        r#"{"ts_ms":1700,"scenario":"prefix-withdrawal","action":"recover"}"#,
        r#"{"ts_ms":2000,"scenario":"trustless-verifier-failure","action":"inject"}"#,
        r#"{"ts_ms":2600,"scenario":"trustless-verifier-failure","action":"recover"}"#,
        r#"{"ts_ms":3000,"scenario":"resolver-brownout","action":"inject"}"#,
        r#"{"ts_ms":3400,"scenario":"resolver-brownout","action":"detect"}"#,
    ]
    .join("\n");
    fs::write(&log_path, format!("{log_entries}\n")).expect("write log");

    let mut report_cmd = cargo_bin_cmd!("xtask");
    let report_output = report_cmd
        .args([
            "soranet-chaos-report",
            "--log",
            log_path.to_str().expect("utf8"),
            "--out",
            summary_path.to_str().expect("utf8"),
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-chaos-report");
    assert!(
        report_output.status.success(),
        "chaos report command failed: {report_output:?}"
    );

    let summary: Value = json::from_reader(fs::File::open(&summary_path).expect("open summary"))
        .expect("decode summary");
    let scenarios = summary
        .get("scenarios")
        .and_then(Value::as_array)
        .cloned()
        .expect("scenarios array");
    let prefix_summary = scenarios
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some("prefix-withdrawal"))
        .cloned()
        .expect("prefix summary");
    assert_eq!(
        Some(300),
        prefix_summary.get("detection_ms").and_then(Value::as_u64),
        "prefix detection delta"
    );
    assert_eq!(
        Some(700),
        prefix_summary.get("recovery_ms").and_then(Value::as_u64),
        "prefix recovery delta"
    );
}
