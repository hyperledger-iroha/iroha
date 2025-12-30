//! Compression telemetry metrics cover incremental usage and JSON helpers.

use std::sync::{Mutex, OnceLock};

use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::{
        CompressionConfig, Header, compression_metrics_delta_json, compression_metrics_json_string,
        compression_metrics_json_value, compression_metrics_reset, compression_metrics_snapshot,
        to_bytes, to_compressed_bytes,
    },
};

#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq, iroha_schema::IntoSchema,
)]
struct TelemetrySample {
    id: u32,
    payload: Vec<u8>,
}

fn metrics_lock() -> &'static Mutex<()> {
    static METRICS_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    METRICS_LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn compression_metrics_snapshot_records_usage() {
    let _guard = metrics_lock().lock().expect("lock metrics");
    compression_metrics_reset();

    let sample = TelemetrySample {
        id: 42,
        payload: (0..64).map(|v| (v as u8).wrapping_mul(3)).collect(),
    };

    let before = compression_metrics_snapshot();
    let compressed = to_compressed_bytes(&sample, Some(CompressionConfig { level: 1 }))
        .expect("compress sample");
    let bare_len = to_bytes(&sample).expect("encode sample").len() - Header::SIZE;
    let body_len = compressed.len() - Header::SIZE;
    let after = compression_metrics_snapshot();

    assert!(
        after.calls > before.calls,
        "compression calls did not increase: before={}, after={}",
        before.calls,
        after.calls
    );
    assert!(
        after.zstd_selected > before.zstd_selected,
        "zstd counter did not increase: before={}, after={}",
        before.zstd_selected,
        after.zstd_selected
    );
    assert!(
        after.bytes_in_total >= before.bytes_in_total + bare_len as u64,
        "bytes_in_total missing bare payload: before={}, after={}, bare_len={}",
        before.bytes_in_total,
        after.bytes_in_total,
        bare_len
    );
    assert!(
        after.bytes_out_total >= before.bytes_out_total + body_len as u64,
        "bytes_out_total missing compressed payload: before={}, after={}, body_len={}",
        before.bytes_out_total,
        after.bytes_out_total,
        body_len
    );
}

#[test]
fn compression_metrics_json_helpers_report_deltas() {
    let _guard = metrics_lock().lock().expect("lock metrics");
    compression_metrics_reset();

    let sample = TelemetrySample {
        id: 7,
        payload: vec![0xAA; 32],
    };

    let before = compression_metrics_json_value();
    let before_map = before
        .as_object()
        .expect("compression metrics should be a JSON object")
        .clone();
    for key in [
        "calls",
        "none_selected",
        "zstd_selected",
        "bytes_in_total",
        "bytes_out_total",
    ] {
        assert!(
            before_map.contains_key(key),
            "missing key {key} in snapshot JSON"
        );
    }

    let _ = to_compressed_bytes(&sample, None).expect("compress without zstd");
    let after = compression_metrics_json_value();
    let delta = compression_metrics_delta_json(&before, &after);
    let delta_map = delta
        .as_object()
        .expect("compression delta should be a JSON object");

    let calls_delta = delta_map
        .get("calls")
        .and_then(|v| v.as_u64())
        .expect("calls delta should be a number");
    assert!(
        calls_delta >= 1,
        "calls delta must be at least 1, got {calls_delta}"
    );

    let none_delta = delta_map
        .get("none_selected")
        .and_then(|v| v.as_u64())
        .expect("none_selected delta should be a number");
    assert!(
        none_delta >= 1,
        "none_selected delta must be at least 1, got {none_delta}"
    );

    let zstd_delta = delta_map
        .get("zstd_selected")
        .and_then(|v| v.as_u64())
        .expect("zstd_selected delta should be a number");
    assert!(
        zstd_delta <= calls_delta,
        "zstd delta cannot exceed total calls"
    );

    for key in ["bytes_in_total", "bytes_out_total"] {
        let value = delta_map
            .get(key)
            .and_then(|v| v.as_u64())
            .expect("byte counters should be numbers");
        assert!(value > 0, "delta for {key} should be positive");
    }

    let json = compression_metrics_json_string();
    assert!(
        json.contains("\"calls\""),
        "snapshot JSON string missing calls key"
    );
}
