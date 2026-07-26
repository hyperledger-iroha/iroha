//! Sanity-check telemetry for the generic codec two-pass path.

#[test]
fn codec_two_pass_records_calls() {
    // Reset and take a baseline snapshot
    norito::codec::adaptive_metrics_reset();
    let before = norito::codec::adaptive_metrics_snapshot();

    // Run encode_adaptive on a couple of inputs. We do not require reencode
    // to trigger deterministically; this test only verifies the call counter.
    let v_small: Vec<u64> = (0..32u64).collect();
    let v_large: Vec<u64> = (0..8192u64).collect();
    let _b1 = norito::codec::encode_adaptive(&v_small);
    let _b2 = norito::codec::encode_adaptive(&v_large);

    let after = norito::codec::adaptive_metrics_snapshot();
    assert!(
        after.calls >= before.calls + 2,
        "calls not incremented by 2"
    );
    // bytes_abs_diff_total is monotonic; we only assert it didn't go backwards.
    assert!(
        after.bytes_abs_diff_total >= before.bytes_abs_diff_total,
        "bytes_abs_diff_total regressed"
    );
}
