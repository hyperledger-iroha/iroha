use std::{fs, path::Path};

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use tempfile::tempdir;

#[test]
fn soranet_gateway_billing_runs_end_to_end() {
    let temp = tempdir().expect("tempdir");
    let out_dir = temp.path().join("billing");
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");

    let usage = workspace_root
        .join("configs")
        .join("soranet")
        .join("gateway_m0")
        .join("billing_usage_sample.json");
    let catalog = workspace_root
        .join("configs")
        .join("soranet")
        .join("gateway_m0")
        .join("meter_catalog.json");
    let guardrails = workspace_root
        .join("configs")
        .join("soranet")
        .join("gateway_m0")
        .join("billing_guardrails.json");

    let mut cmd = cargo_bin_cmd!("xtask");
    let output = cmd
        .current_dir(workspace_root)
        .args([
            "soranet-gateway-billing",
            "--usage",
            usage.to_str().expect("utf8"),
            "--catalog",
            catalog.to_str().expect("utf8"),
            "--guardrails",
            guardrails.to_str().expect("utf8"),
            "--output-dir",
            out_dir.to_str().expect("utf8"),
            "--payer",
            "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
            "--treasury",
            "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
            "--asset",
            "4cuvDVPuLBKJyN6dPbRQhmLh68sU",
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-gateway-billing");
    assert!(
        output.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let invoice_bytes =
        fs::read(out_dir.join("billing_invoice.json")).expect("invoice JSON exists");
    let invoice: Value = json::from_slice(&invoice_bytes).expect("invoice parses");
    assert_eq!(
        invoice["totals"]["total_micros"],
        Value::from(124_203_000u64),
        "invoice total changed"
    );
    assert_eq!(
        invoice["guardrails"]["alert_triggered"],
        Value::from(false),
        "unexpected alert trigger"
    );
    assert_eq!(
        invoice["guardrails"]["hard_cap_exceeded"],
        Value::from(false),
        "hard cap exceeded unexpectedly"
    );
    let normalized = invoice["normalized_entries"]
        .as_array()
        .expect("normalized entries array");
    assert_eq!(normalized.len(), 6, "normalized usage entries changed");
    assert!(
        invoice["merge_notes"]
            .as_array()
            .expect("merge notes array")
            .is_empty(),
        "merge notes should be empty for the sample"
    );

    let parquet_path = out_dir.join("billing_invoice.parquet");
    let file = fs::File::open(&parquet_path).expect("parquet exists");
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .expect("parquet reader")
        .build()
        .expect("parquet build");
    let mut rows = 0;
    for batch in reader {
        let batch = batch.expect("parquet batch");
        rows += batch.num_rows();
    }
    assert_eq!(rows, 6, "parquet row count mismatch");

    let ledger_bytes =
        fs::read(out_dir.join("billing_ledger_projection.json")).expect("ledger exists");
    let ledger: Value = json::from_slice(&ledger_bytes).expect("ledger parses");
    assert_eq!(
        ledger["total_micros"],
        Value::from(124_203_000u64),
        "ledger total changed"
    );
    assert_eq!(
        ledger["asset_definition"],
        Value::from("4cuvDVPuLBKJyN6dPbRQhmLh68sU"),
        "ledger asset definition mismatch"
    );
}
