use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use tempfile::tempdir;

#[test]
fn soranet_billing_m0_pack_includes_exports_and_projection() {
    let temp = tempdir().expect("tempdir");
    let out_dir = temp.path().join("billing");

    let mut cmd = cargo_bin_cmd!("xtask");
    let output = cmd
        .args([
            "soranet-gateway-billing-m0",
            "--output-dir",
            out_dir.to_str().expect("utf8"),
            "--billing-period",
            "2026-11",
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

    let summary_bytes = fs::read(out_dir.join("billing_m0_summary.json")).expect("summary exists");
    let summary: Value = json::from_slice(&summary_bytes).expect("summary decodes");
    assert!(
        summary
            .get("meter_catalog_csv_path")
            .and_then(Value::as_str)
            .is_some(),
        "csv path recorded"
    );
    assert!(
        summary
            .get("meter_catalog_parquet_path")
            .and_then(Value::as_str)
            .is_some(),
        "parquet path recorded"
    );

    let catalog_bytes =
        fs::read(out_dir.join("billing_meter_catalog.json")).expect("catalog exists");
    let catalog: Value = json::from_slice(&catalog_bytes).expect("catalog decodes");
    let meters = catalog["meters"].as_array().expect("meters array present");
    assert_eq!(meters.len(), 6, "meter count");
    assert!(
        meters
            .iter()
            .any(|meter| meter["id"].as_str() == Some("storage.gib_month")),
        "storage meter present"
    );

    let csv = fs::read_to_string(out_dir.join("billing_meter_catalog.csv")).expect("csv exists");
    assert!(csv.contains("http.request"), "csv includes http.request");
    assert!(csv.lines().count() > 5, "csv has data rows");

    let parquet =
        fs::File::open(out_dir.join("billing_meter_catalog.parquet")).expect("parquet exists");
    let reader = ParquetRecordBatchReaderBuilder::try_new(parquet)
        .expect("parquet reader builder")
        .build()
        .expect("parquet reader");
    let mut total_rows = 0;
    for batch in reader {
        total_rows += batch.expect("batch").num_rows();
    }
    assert!(total_rows >= 24, "parquet rows present");

    let projection_bytes =
        fs::read(out_dir.join("billing_ledger_projection.json")).expect("projection exists");
    let projection: Value = json::from_slice(&projection_bytes).expect("projection decodes");
    assert!(
        projection["totals"]["gross_micros"]
            .as_u64()
            .expect("gross exists")
            > 0,
        "gross total present"
    );
    assert!(
        projection["usage"].as_array().expect("usage array").len() >= 5,
        "usage lines present"
    );

    let guardrails =
        fs::read_to_string(out_dir.join("billing_guardrails.yaml")).expect("guardrails exist");
    assert!(guardrails.contains("alerts:"), "guardrails include alerts");
}
