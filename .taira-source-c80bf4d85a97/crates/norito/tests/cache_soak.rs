//! Soak tests for Norito columnar cache guards and telemetry.

use norito::{
    Error,
    columnar::{
        MAX_CACHE_ROWS, adaptive_metrics_reset, adaptive_metrics_snapshot, encode_opt_str_column,
        view_opt_str_column,
    },
};

#[test]
fn cache_metrics_accumulate_over_soak() {
    adaptive_metrics_reset();
    let rows: Vec<Option<&str>> = vec![
        Some("alpha"),
        None,
        Some("beta"),
        Some("gamma"),
        Some("delta"),
    ];
    let (bytes, _present) = encode_opt_str_column(&rows);
    let before = adaptive_metrics_snapshot();
    for _ in 0..5 {
        let view = view_opt_str_column(&bytes, rows.len()).expect("view");
        assert_eq!(view.len(), rows.len());
        // Touch a few rows to ensure we exercise cached lookups.
        assert_eq!(view.get(0).expect("row").unwrap(), "alpha");
        assert!(view.get(1).expect("row").is_none());
    }
    let snap = adaptive_metrics_snapshot();
    let builds_delta = snap.cache_builds.saturating_sub(before.cache_builds);
    assert!(
        builds_delta >= 1,
        "expected the cache to build at least once during the soak (Δ={builds_delta})"
    );
    let rows_delta = snap
        .cache_rows_total
        .saturating_sub(before.cache_rows_total);
    assert!(
        rows_delta >= rows.len() as u64,
        "rows_total should record cached rows (Δ={rows_delta})"
    );
    assert_eq!(snap.cache_rejects, before.cache_rejects);
    assert_eq!(snap.cache_reject_rows_total, before.cache_reject_rows_total);
}

#[test]
fn cache_rejects_malicious_presence_stream() {
    adaptive_metrics_reset();
    let n_rows = MAX_CACHE_ROWS + 1;
    let bit_bytes = n_rows.div_ceil(8);
    let mut bytes = vec![0u8; bit_bytes];
    let mis4 = bit_bytes & 3;
    if mis4 != 0 {
        bytes.extend(std::iter::repeat_n(0u8, 4 - mis4));
    }
    bytes.extend_from_slice(&0u32.to_le_bytes());
    let err = match view_opt_str_column(&bytes, n_rows) {
        Ok(_) => panic!("rows limit should reject oversize payload"),
        Err(err) => err,
    };
    match err {
        Error::UnsupportedFeature(msg) => {
            assert!(msg.contains("cache"), "unexpected message: {msg}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
    let snap = adaptive_metrics_snapshot();
    assert_eq!(snap.cache_rejects, 1);
    assert_eq!(snap.cache_reject_rows_total, n_rows as u64);
}
