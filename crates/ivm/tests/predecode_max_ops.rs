//! Ensure pre-decode enforces a hard per-entry instruction cap.
#[test]
fn predecode_max_ops_cap() {
    let _lease = crate::predecode_test_support::exclusive();
    // Set a very low cap to trigger the guard quickly.
    let baseline = ivm::ivm_cache::cache_limits();
    let _guard = ivm::ivm_cache::CacheLimitsGuard::new(ivm::ivm_cache::CacheLimits {
        max_decoded_ops: 3,
        ..baseline
    });
    // A stream exactly at the configured cap must still be admitted.
    let word = ivm::encoding::wide::encode_halt();
    let code_at_cap = word.to_le_bytes().repeat(3);
    let decoded = ivm::ivm_cache::IvmCache::decode_stream(&code_at_cap)
        .expect("stream exactly at the decoded-op cap");
    assert_eq!(decoded.len(), 3);

    // The first instruction beyond the cap must fail before it is decoded.
    let code_over_cap = word.to_le_bytes().repeat(4);
    let res = ivm::ivm_cache::IvmCache::decode_stream(&code_over_cap);
    assert!(
        matches!(res, Err(ivm::VMError::DecodeError)),
        "expected cap violation"
    );
}
