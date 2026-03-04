//! Ensure the pre-decode cache evicts by approximate byte budget, not only count.

#[test]
fn predecode_memory_budget_eviction() {
    // Large entry-cap so only byte budget triggers eviction.
    // Use a small byte budget so that a second insert forces eviction.
    let baseline = ivm::ivm_cache::cache_limits();
    let _guard = ivm::ivm_cache::CacheLimitsGuard::new(ivm::ivm_cache::CacheLimits {
        max_bytes: 64,
        ..baseline
    });

    let mut cache = ivm::ivm_cache::IvmCache::new(10);

    // Build a code object with 1 instruction (HALT)
    let halt = ivm::encoding::wide::encode_halt();
    let mut code1 = Vec::new();
    code1.extend_from_slice(&halt.to_le_bytes());

    // Another with 4 instructions
    let mut code4 = Vec::new();
    for _ in 0..4 {
        code4.extend_from_slice(&halt.to_le_bytes());
    }

    // Insert first entry
    let _ = cache.get_or_predecode(&code1, 1, 0).expect("decode 1");
    // Insert second entry — combined size should exceed 64 bytes and evict the oldest
    let _ = cache.get_or_predecode(&code4, 1, 0).expect("decode 4");

    // The first key should have been evicted; re-requesting should miss (and reinsert)
    let before = cache.counters();
    let _ = cache
        .get_or_predecode(&code1, 1, 0)
        .expect("decode 1 again");
    let after = cache.counters();
    assert!(after.1 > before.1, "expected a miss for evicted entry");
}
