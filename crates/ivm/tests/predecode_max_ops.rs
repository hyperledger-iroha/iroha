//! Ensure pre-decode enforces a hard per-entry instruction cap.

#[test]
fn predecode_max_ops_cap() {
    // Set a very low cap to trigger the guard quickly.
    let baseline = ivm::ivm_cache::cache_limits();
    let _guard = ivm::ivm_cache::CacheLimitsGuard::new(ivm::ivm_cache::CacheLimits {
        max_decoded_ops: 3,
        ..baseline
    });

    // Build 4 HALT instructions (each 32-bit). Decoder should stop after 3 ops.
    let word = ivm::encoding::wide::encode_halt();
    let mut code = Vec::new();
    for _ in 0..4 {
        code.extend_from_slice(&word.to_le_bytes());
    }

    let res = ivm::ivm_cache::IvmCache::decode_stream(&code);
    assert!(
        matches!(res, Err(ivm::VMError::DecodeError)),
        "expected cap violation"
    );
}
