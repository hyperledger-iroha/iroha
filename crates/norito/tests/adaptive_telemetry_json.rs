//! Check JSON export helpers for adaptive telemetry.

#[test]
#[cfg(feature = "json")]
fn columnar_json_keys_present() {
    norito::columnar::adaptive_metrics_reset();
    // Trigger one encode on the small-N path to touch counters
    let rows: Vec<(u64, &str, bool)> = vec![(1, "a", true), (2, "b", false)];
    let _ = norito::columnar::encode_rows_u64_str_bool_adaptive(&rows);
    let v = norito::columnar::adaptive_metrics_json_value();
    // Ensure object with required keys
    let obj = v.as_object().expect("object");
    assert!(obj.contains_key("aos_selected"));
    assert!(obj.contains_key("ncb_selected"));
    assert!(obj.contains_key("probes"));
    assert!(obj.contains_key("bytes_saved_total"));
}

#[test]
#[cfg(feature = "json")]
fn codec_json_keys_present() {
    norito::codec::adaptive_metrics_reset();
    let small: Vec<u64> = (0..16u64).collect();
    let _ = norito::codec::encode_adaptive(&small);
    let v = norito::codec::adaptive_metrics_json_value();
    let obj = v.as_object().expect("object");
    assert!(obj.contains_key("calls"));
    assert!(obj.contains_key("reencodes"));
    assert!(obj.contains_key("bytes_abs_diff_total"));
}
