#![cfg(feature = "cli")]
use std::{
    env,
    error::Error,
    fs,
    path::PathBuf,
    process::Command,
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iroha_data_model::{
    name::Name,
    taikai::{
        CEK_ROTATION_RECEIPT_VERSION_V1, CekRotationReceiptV1, TaikaiEventId, TaikaiStreamId,
    },
};
use norito::json;
use tempfile::tempdir;

fn taikai_publisher_example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("sdk/examples/taikai_publisher")
}

#[test]
fn taikai_viewer_emits_metrics_and_summary() -> Result<(), Box<dyn Error>> {
    let python = env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());
    if Command::new(&python).arg("--version").output().is_err() {
        eprintln!("skipping taikai_viewer test because `{python}` is unavailable");
        return Ok(());
    }

    let taikai_car_path = assert_cmd::cargo::cargo_bin!("taikai_car");
    let taikai_viewer_path = {
        let candidate = taikai_car_path
            .parent()
            .expect("taikai_car parent dir")
            .join("taikai_viewer");
        if candidate.exists() {
            candidate
        } else {
            let status = Command::new("cargo")
                .args([
                    "build",
                    "-p",
                    "sorafs_orchestrator",
                    "--bin",
                    "taikai_viewer",
                ])
                .env("CARGO_NET_OFFLINE", "true")
                .status()?;
            assert!(
                status.success(),
                "cargo build taikai_viewer failed: {status}"
            );
            taikai_car_path
                .parent()
                .expect("taikai_car parent dir")
                .join("taikai_viewer")
        }
    };
    let sample_dir = taikai_publisher_example_dir();
    let script = sample_dir.join("bundle_sample.py");
    let config_primary = sample_dir.join("sample_config.json");
    let config_secondary = sample_dir.join("sample_config_720p.json");
    let temp = tempdir()?;
    let out_dir = temp.path().join("publisher");
    let summary_path = out_dir.join("publisher_summary.json");

    let output = Command::new(&python)
        .arg(&script)
        .arg("--config")
        .arg(&config_primary)
        .arg("--config")
        .arg(&config_secondary)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--summary-json")
        .arg(&summary_path)
        .env(
            "TAIKAI_CAR_BIN",
            taikai_car_path.to_str().expect("taikai_car path utf8"),
        )
        .output()?;
    assert!(
        output.status.success(),
        "bundle_sample.py failed: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let envelope_primary = out_dir.join("sample_segment_0001.norito");
    let car_primary = out_dir.join("sample_segment_0001.car");
    let envelope_ladder = out_dir.join("sample_segment_0001_720p.norito");
    let car_ladder = out_dir.join("sample_segment_0001_720p.car");
    for required in [
        &envelope_primary,
        &car_primary,
        &envelope_ladder,
        &car_ladder,
    ] {
        assert!(required.exists(), "missing {}", required.display());
    }

    let cek_path = temp.path().join("cek_receipt.norito");
    let receipt = CekRotationReceiptV1 {
        schema_version: CEK_ROTATION_RECEIPT_VERSION_V1,
        event_id: TaikaiEventId::new(Name::from_str("global-keynote")?),
        stream_id: TaikaiStreamId::new(Name::from_str("stage-a")?),
        kms_profile: "demo-kms".to_string(),
        new_wrap_key_label: "wrap-v2".to_string(),
        previous_wrap_key_label: Some("wrap-v1".to_string()),
        hkdf_salt: [0x11; 32],
        effective_segment_sequence: 42,
        issued_at_unix: SystemTime::now()
            .checked_sub(Duration::from_secs(120))
            .unwrap_or_else(SystemTime::now)
            .duration_since(UNIX_EPOCH)?
            .as_secs(),
        notes: Some("test receipt".to_string()),
    };
    let receipt_buf = norito::to_bytes(&receipt).expect("encode cek receipt");
    fs::write(&cek_path, &receipt_buf)?;

    let metrics_out = temp.path().join("metrics.prom");
    let viewer_summary = temp.path().join("viewer_summary.json");

    let viewer_output = Command::new(&taikai_viewer_path)
        .arg("--segment")
        .arg(format!(
            "envelope={},car={}",
            envelope_primary.display(),
            car_primary.display()
        ))
        .arg("--segment")
        .arg(format!(
            "envelope={},car={}",
            envelope_ladder.display(),
            car_ladder.display()
        ))
        .arg("--cluster")
        .arg("demo-cluster")
        .arg("--lane")
        .arg("lane-main")
        .arg("--rebuffer-events")
        .arg("2")
        .arg("--pq-health")
        .arg("87.5")
        .arg("--cek-receipt")
        .arg(cek_path.to_str().expect("cek path utf8"))
        .arg("--cek-fetch-ms")
        .arg("25")
        .arg("--metrics-out")
        .arg(metrics_out.to_str().expect("metrics path utf8"))
        .arg("--summary-out")
        .arg(viewer_summary.to_str().expect("summary path utf8"))
        .output()?;
    assert!(
        viewer_output.status.success(),
        "taikai_viewer failed: {}\nstdout: {}\nstderr: {}",
        viewer_output.status,
        String::from_utf8_lossy(&viewer_output.stdout),
        String::from_utf8_lossy(&viewer_output.stderr)
    );

    let metrics_text = fs::read_to_string(&metrics_out)?;
    assert!(
        metrics_text.contains(
            "taikai_viewer_playback_segments_total{cluster=\"demo-cluster\",stream=\"stage-a\"} 2"
        ),
        "playback counter missing: {metrics_text}"
    );
    assert!(
        metrics_text.contains(
            "taikai_viewer_rebuffer_events_total{cluster=\"demo-cluster\",stream=\"stage-a\"} 2"
        ),
        "rebuffer counter missing: {metrics_text}"
    );
    assert!(
        metrics_text.contains("taikai_viewer_pq_circuit_health{cluster=\"demo-cluster\"} 87.5"),
        "PQ health gauge missing: {metrics_text}"
    );
    assert!(
        metrics_text.contains("taikai_viewer_cek_rotation_seconds_ago{lane=\"lane-main\"}"),
        "CEK rotation gauge missing: {metrics_text}"
    );

    let summary_raw = fs::read(&viewer_summary)?;
    let summary: json::Value = json::from_slice(&summary_raw)?;
    assert_eq!(
        summary["pq_health_percent"],
        json::Value::from(87.5f64),
        "summary pq_health_percent mismatch: {summary:?}"
    );
    assert_eq!(
        summary["rebuffer_events_applied"],
        json::Value::from(2u64),
        "summary rebuffer mismatch: {summary:?}"
    );
    let segments = summary["segments"].as_array().expect("segments array");
    assert_eq!(
        segments.len(),
        2,
        "expected two segments in summary: {summary:?}"
    );
    let cek = summary["cek"].as_object().expect("cek object in summary");
    assert_eq!(cek["duration_ms"], json::Value::from(25u64));

    Ok(())
}
