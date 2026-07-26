//! Prints a snapshot of adaptive AoS/NCB telemetry after a small mixed run.
//!
//! Run:
//! - cargo bench -p norito --bench adaptive_telemetry
//! - With times: cargo bench -p norito --bench adaptive_telemetry --features adaptive-telemetry

use norito::{
    codec,
    columnar::{
        EnumBorrow, adaptive_metrics_reset, adaptive_metrics_snapshot,
        encode_rows_u64_bytes_bool_adaptive, encode_rows_u64_bytes_u32_bool_adaptive,
        encode_rows_u64_enum_bool_adaptive, encode_rows_u64_optstr_bool_adaptive,
        encode_rows_u64_optu32_bool_adaptive, encode_rows_u64_str_bool_adaptive,
        encode_rows_u64_str_u32_bool_adaptive, encode_rows_u64_u32_bool_adaptive,
    },
};

fn main() {
    // Ensure clean counters for this run
    adaptive_metrics_reset();

    // Build a few small datasets (<= small_n) to exercise the two-pass path.
    let mut sink: usize = 0; // prevent dead-code-elim

    // u64, &str, bool
    let rows_aos: Vec<(u64, &str, bool)> = vec![
        (1, "alpha", true),
        (2, "beta", false),
        (3, "gamma", true),
        (4, "delta", false),
    ];
    let rows_ncb: Vec<(u64, &str, bool)> = (0..16u64)
        .map(|i| (i, if i % 3 == 0 { "alpha" } else { "beta" }, i % 2 == 0))
        .collect();
    sink ^= encode_rows_u64_str_bool_adaptive(&rows_aos).len();
    sink ^= encode_rows_u64_str_bool_adaptive(&rows_ncb).len();

    // u64, Option<&str>, bool
    let rows_optstr: Vec<(u64, Option<&str>, bool)> = vec![
        (10, Some("alice"), true),
        (11, None, false),
        (12, Some("bob"), true),
        (13, Some("alice"), false),
    ];
    sink ^= encode_rows_u64_optstr_bool_adaptive(&rows_optstr).len();

    // u64, Option<u32>, bool
    let rows_optu32: Vec<(u64, Option<u32>, bool)> =
        vec![(1, Some(10), true), (2, None, false), (3, Some(11), true)];
    sink ^= encode_rows_u64_optu32_bool_adaptive(&rows_optu32).len();

    // u64, &[u8], bool
    let rows_bytes: Vec<(u64, &[u8], bool)> =
        vec![(1, b"abc".as_slice(), true), (2, b"abc".as_slice(), false)];
    sink ^= encode_rows_u64_bytes_bool_adaptive(&rows_bytes).len();

    // u64, u32, bool
    let rows_u32: Vec<(u64, u32, bool)> = vec![(1, 100, true), (2, 101, false), (3, 105, true)];
    sink ^= encode_rows_u64_u32_bool_adaptive(&rows_u32).len();

    // u64, &str, u32, bool
    let rows_str_u32: Vec<(u64, &str, u32, bool)> =
        vec![(1, "ax", 7, true), (2, "ay", 8, false), (3, "ax", 9, true)];
    sink ^= encode_rows_u64_str_u32_bool_adaptive(&rows_str_u32).len();

    // u64, &[u8], u32, bool
    let rows_bytes_u32: Vec<(u64, &[u8], u32, bool)> = vec![
        (1, b"aaaa".as_slice(), 1, true),
        (2, b"bbbb".as_slice(), 2, false),
        (3, b"aaaa".as_slice(), 3, true),
    ];
    sink ^= encode_rows_u64_bytes_u32_bool_adaptive(&rows_bytes_u32).len();

    // u64, enum, bool
    let rows_enum: Vec<(u64, EnumBorrow<'_>, bool)> = vec![
        (1, EnumBorrow::Name("alice"), true),
        (2, EnumBorrow::Code(7), false),
        (3, EnumBorrow::Name("alice"), true),
    ];
    sink ^= encode_rows_u64_enum_bool_adaptive(&rows_enum).len();

    // Grab snapshot and print a compact summary
    let snap = adaptive_metrics_snapshot();
    println!(
        "telemetry: sink_bytes={} aos_selected={} ncb_selected={} probes={} bytes_saved_total={}",
        sink, snap.aos_selected, snap.ncb_selected, snap.probes, snap.bytes_saved_total
    );
    #[cfg(feature = "json")]
    println!(
        "telemetry_json: {}",
        norito::columnar::adaptive_metrics_json_string()
    );

    #[cfg(feature = "adaptive-telemetry")]
    println!(
        "telemetry_times: aos_time_ns_total={} ncb_time_ns_total={} avg_aos_ns={} avg_ncb_ns={}",
        snap.aos_time_ns_total,
        snap.ncb_time_ns_total,
        if snap.probes > 0 {
            snap.aos_time_ns_total / snap.probes
        } else {
            0
        },
        if snap.probes > 0 {
            snap.ncb_time_ns_total / snap.probes
        } else {
            0
        },
    );

    // Also exercise codec::encode_adaptive on a mixed set and print its bucket
    codec::adaptive_metrics_reset();
    let big_vec: Vec<u64> = (0..16384u64).collect();
    let small_vec: Vec<u64> = (0..32u64).collect();
    let _b1 = codec::encode_adaptive(&big_vec);
    let _b2 = codec::encode_adaptive(&small_vec);
    let _b3 = codec::encode_adaptive(&rows_u32);

    let cs = codec::adaptive_metrics_snapshot();
    println!(
        "codec_telemetry: calls={} reencodes={} bytes_abs_diff_total={}",
        cs.calls, cs.reencodes, cs.bytes_abs_diff_total
    );
    #[cfg(feature = "json")]
    println!(
        "codec_telemetry_json: {}",
        codec::adaptive_metrics_json_string()
    );
    #[cfg(feature = "adaptive-telemetry")]
    println!(
        "codec_telemetry_times: pass1_time_ns_total={} pass2_time_ns_total={}",
        cs.pass1_time_ns_total, cs.pass2_time_ns_total
    );

    // Compression telemetry snapshot: run a couple of headered encodes
    let big_text = "a".repeat(1024 * 8);
    let _hb1 = norito::core::to_bytes_auto(&big_text).expect("to_bytes_auto big");
    let big_bytes: Vec<u8> = vec![0u8; 1024 * 16];
    let _hb2 = norito::core::to_bytes_auto(&big_bytes).expect("to_bytes_auto bytes");
    #[cfg(feature = "json")]
    println!(
        "compression_telemetry_json: {}",
        norito::core::compression_metrics_json_string()
    );
}
