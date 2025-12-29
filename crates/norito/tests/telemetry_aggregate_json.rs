//! Check aggregated telemetry JSON contains expected keys.

#[test]
#[cfg(feature = "json")]
fn aggregated_has_keys() {
    norito::telemetry::reset_all();
    // Touch all buckets
    let rows: Vec<(u64, &str, bool)> = vec![(1, "a", true), (2, "b", false)];
    let _ = norito::columnar::encode_rows_u64_str_bool_adaptive(&rows);
    let small: Vec<u64> = (0..8u64).collect();
    let _ = norito::codec::encode_adaptive(&small);
    let _ = norito::core::to_bytes_auto(&"hello".to_string()).expect("to_bytes_auto");

    let v = norito::telemetry::snapshot_json_value();
    let obj = v.as_object().expect("object");
    assert!(obj.contains_key("codec"));
    assert!(obj.contains_key("compression"));
    #[cfg(feature = "columnar")]
    assert!(obj.contains_key("columnar"));
}
