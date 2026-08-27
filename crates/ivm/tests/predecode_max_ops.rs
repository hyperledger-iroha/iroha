//! Ensure pre-decode enforces a hard per-entry instruction cap.
#[test]
fn predecode_max_ops_cap() {
    let _lease = crate::predecode_test_support::exclusive();
    // Warm a cache under a permissive cap before tightening it. Cached streams
    // must remain subject to the current admission limit.
    let baseline = ivm::ivm_cache::cache_limits();
    let _guard = ivm::ivm_cache::CacheLimitsGuard::new(ivm::ivm_cache::CacheLimits {
        max_decoded_ops: 8,
        ..baseline
    });
    let word = ivm::encoding::wide::encode_halt();
    let code_over_cap = word.to_le_bytes().repeat(4);
    let mut cache = ivm::ivm_cache::IvmCache::new(2);
    cache
        .get_or_predecode(&code_over_cap)
        .expect("warm stream under permissive cap");
    ivm::ivm_cache::configure_limits(ivm::ivm_cache::CacheLimits {
        max_decoded_ops: 3,
        ..baseline
    });

    // A stream exactly at the configured cap must still be admitted.
    let code_at_cap = word.to_le_bytes().repeat(3);
    let decoded = ivm::ivm_cache::IvmCache::decode_stream(&code_at_cap)
        .expect("stream exactly at the decoded-op cap");
    assert_eq!(decoded.len(), 3);

    // The first instruction beyond the cap must fail before it is decoded.
    let res = ivm::ivm_cache::IvmCache::decode_stream(&code_over_cap);
    assert!(
        matches!(res, Err(ivm::VMError::DecodeError)),
        "expected cap violation"
    );
    let before = cache.counters();
    assert!(
        matches!(
            cache.get_or_predecode(&code_over_cap),
            Err(ivm::VMError::DecodeError)
        ),
        "cached stream must not bypass a tightened cap"
    );
    let after = cache.counters();
    assert!(after.1 > before.1, "oversized cached hit becomes a miss");
    assert!(after.2 > before.2, "oversized cached hit is evicted");
}
