//! Ensure the decoded-op limit controls cache retention without changing decoding.

use std::sync::Arc;

fn halt_stream(ops: usize) -> Vec<u8> {
    ivm::encoding::wide::encode_halt().to_le_bytes().repeat(ops)
}

#[test]
fn decode_stream_ignores_cache_retention_cap() {
    let _lease = crate::predecode_test_support::exclusive();
    let baseline = ivm::ivm_cache::cache_limits();
    let _guard = ivm::ivm_cache::CacheLimitsGuard::new(ivm::ivm_cache::CacheLimits {
        max_decoded_ops: 3,
        ..baseline
    });

    let decoded = ivm::ivm_cache::IvmCache::decode_stream(&halt_stream(4))
        .expect("cache retention cap must not reject valid code");
    assert_eq!(decoded.len(), 4);
}

#[test]
fn stream_over_retention_cap_decodes_without_being_cached() {
    let _lease = crate::predecode_test_support::exclusive();
    let baseline = ivm::ivm_cache::cache_limits();
    let _guard = ivm::ivm_cache::CacheLimitsGuard::new(ivm::ivm_cache::CacheLimits {
        max_decoded_ops: 3,
        ..baseline
    });
    let code = halt_stream(4);
    let mut cache = ivm::ivm_cache::IvmCache::new(2);

    let first = cache
        .get_or_predecode(&code)
        .expect("cold over-cap stream must decode");
    assert_eq!(first.len(), 4);
    assert_eq!(cache.counters(), (0, 1, 0));

    let second = cache
        .get_or_predecode(&code)
        .expect("repeated over-cap stream must decode");
    assert_eq!(second.len(), 4);
    assert!(!Arc::ptr_eq(&first, &second));
    assert_eq!(cache.counters(), (0, 2, 0));
}

#[test]
fn stream_at_retention_cap_is_cached() {
    let _lease = crate::predecode_test_support::exclusive();
    let baseline = ivm::ivm_cache::cache_limits();
    let _guard = ivm::ivm_cache::CacheLimitsGuard::new(ivm::ivm_cache::CacheLimits {
        max_decoded_ops: 3,
        ..baseline
    });
    let code = halt_stream(3);
    let mut cache = ivm::ivm_cache::IvmCache::new(2);

    let first = cache
        .get_or_predecode(&code)
        .expect("stream exactly at cap must decode");
    assert_eq!(cache.counters(), (0, 1, 0));

    let second = cache
        .get_or_predecode(&code)
        .expect("stream exactly at cap must hit cache");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(cache.counters(), (1, 1, 0));
}

#[test]
fn tightened_retention_cap_evicts_warm_entry_then_decodes_uncached() {
    let _lease = crate::predecode_test_support::exclusive();
    let baseline = ivm::ivm_cache::cache_limits();
    let _guard = ivm::ivm_cache::CacheLimitsGuard::new(ivm::ivm_cache::CacheLimits {
        max_decoded_ops: 8,
        ..baseline
    });
    let code = halt_stream(4);
    let mut cache = ivm::ivm_cache::IvmCache::new(2);
    let warm = cache
        .get_or_predecode(&code)
        .expect("warm stream under permissive cap");
    assert_eq!(cache.counters(), (0, 1, 0));

    ivm::ivm_cache::configure_limits(ivm::ivm_cache::CacheLimits {
        max_decoded_ops: 3,
        ..baseline
    });

    let first_uncached = cache
        .get_or_predecode(&code)
        .expect("tightened cap must not reject a warmed stream");
    assert_eq!(first_uncached.len(), 4);
    assert!(!Arc::ptr_eq(&warm, &first_uncached));
    assert_eq!(cache.counters(), (0, 2, 1));

    let second_uncached = cache
        .get_or_predecode(&code)
        .expect("evicted over-cap stream must keep decoding");
    assert!(!Arc::ptr_eq(&first_uncached, &second_uncached));
    assert_eq!(cache.counters(), (0, 3, 1));
}
