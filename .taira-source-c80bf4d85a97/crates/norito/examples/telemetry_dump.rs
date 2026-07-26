//! Print Norito telemetry snapshots (JSON) after a small mixed run.
//!
//! Run:
//! - cargo run -p norito --example telemetry_dump
//! - With pass timings in snapshots: cargo run -p norito --example telemetry_dump --features adaptive-telemetry

fn main() {
    // Columnar small-N two-pass paths
    let rows: Vec<(u64, &str, bool)> = vec![
        (1, "alpha", true),
        (2, "beta", false),
        (3, "alpha", true),
        (4, "beta", false),
    ];
    let _ = norito::columnar::encode_rows_u64_str_bool_adaptive(&rows);

    // codec::encode_adaptive (bare)
    let small: Vec<u64> = (0..32u64).collect();
    let _b = norito::codec::encode_adaptive(&small);

    // Headered adaptive compression path
    let big_text = "a".repeat(12 * 1024);
    let _hb = norito::core::to_bytes_auto(&big_text).expect("to_bytes_auto");

    // Print JSON snapshots (available under default features)
    #[cfg(feature = "json")]
    {
        println!(
            "columnar_telemetry_json: {}",
            norito::columnar::adaptive_metrics_json_string()
        );
        println!(
            "codec_telemetry_json: {}",
            norito::codec::adaptive_metrics_json_string()
        );
        println!(
            "compression_telemetry_json: {}",
            norito::core::compression_metrics_json_string()
        );
    }

    // Aggregated snapshot
    #[cfg(feature = "json")]
    println!(
        "aggregated_telemetry_json: {}",
        norito::telemetry::snapshot_json_string()
    );
}
