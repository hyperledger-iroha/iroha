//! Sanity-check adaptive AoS/NCB telemetry under a controlled small-N sample.

use norito::columnar::{
    adaptive_metrics_reset, adaptive_metrics_snapshot, encode_ncb_u64_str_bool,
    encode_rows_u64_str_bool_adaptive,
};

#[test]
fn adaptive_two_pass_records_probe_and_bytes_saved() {
    // Build a tiny dataset (<= small_n) to trigger the two-pass probe path.
    // Two distinct short strings usually favor AoS or NCB depending on content;
    // we compute the exact diff locally and assert telemetry increased by at least that.
    let rows: Vec<(u64, &str, bool)> = vec![(1, "alpha", true), (2, "beta", false)];
    // Compute AoS and NCB lengths locally
    let aos_len = norito::aos::encode_rows_u64_str_bool(&rows).len();
    let ncb_len = encode_ncb_u64_str_bool(&rows).len();
    let diff = aos_len.abs_diff(ncb_len) as u64;

    // Reset counters (best-effort: other tests may race; use >= assertions below)
    adaptive_metrics_reset();
    let before = adaptive_metrics_snapshot();

    // Trigger the adaptive encoder (two-pass probe)
    let _bytes = encode_rows_u64_str_bool_adaptive(&rows);

    let after = adaptive_metrics_snapshot();
    // At least one probe and one selection must have been recorded
    assert!(after.probes > before.probes, "probes not incremented");
    let sel_before = before.aos_selected + before.ncb_selected;
    let sel_after = after.aos_selected + after.ncb_selected;
    assert!(sel_after > sel_before, "selection counters not incremented");
    // Bytes saved must grow by at least the diff compared to before snapshot
    assert!(
        after.bytes_saved_total >= before.bytes_saved_total + diff,
        "bytes_saved_total expected >= {} (Δ before->after), got before={} after={}",
        diff,
        before.bytes_saved_total,
        after.bytes_saved_total
    );
}
