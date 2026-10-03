//! Configuration publication across synchronous eviction callbacks.

use std::sync::atomic::AtomicBool;

use super::*;
use crate::cache_memory::{CacheEvictionRegistration, register_cache_evictor};

const INITIAL: CacheLimits = CacheLimits {
    capacity: 67,
    max_bytes: 64 * 1024 * 1024,
    max_decoded_ops: 500,
};
const OUTER: CacheLimits = CacheLimits {
    capacity: 71,
    max_bytes: 1024 * 1024,
    max_decoded_ops: 700,
};
const REENTRANT: CacheLimits = CacheLimits {
    capacity: 5,
    max_bytes: 2 * 1024 * 1024,
    max_decoded_ops: 0,
};

fn assert_published(expected: CacheLimits) {
    assert_eq!(cache_limits(), expected);
    assert_eq!(configured_max_bytes(), expected.max_bytes);
    assert_eq!(
        configured_max_decoded_ops(),
        normalize_limits(expected).max_decoded_ops
    );
    assert_eq!(
        crate::cache_memory::memory_stats().limit_bytes,
        expected.max_bytes
    );
    let cache = GLOBAL_CACHE.get().expect("initialized global cache");
    assert_eq!(
        cache.total_capacity.load(Ordering::Relaxed),
        expected.capacity
    );
    let shards = cache.shards.read().expect("shard list");
    for (index, shard) in shards.iter().enumerate() {
        let shard = shard.lock().expect("decoded cache shard");
        assert_eq!(
            shard.cap,
            shard_share(expected.capacity, shards.len(), index)
        );
        assert_eq!(
            shard.max_bytes,
            shard_share(expected.max_bytes, shards.len(), index)
        );
    }
}

fn register_once(
    callback: impl Fn() + Send + Sync + 'static,
) -> (CacheEvictionRegistration, Arc<AtomicBool>) {
    let called = Arc::new(AtomicBool::new(false));
    let callback_called = Arc::clone(&called);
    let owner = std::thread::current().id();
    let registration = register_cache_evictor(move || {
        // Other tests may directly invoke the shared eviction registry without
        // taking the configuration guard. Only this writer owns our callback.
        if std::thread::current().id() == owner && !callback_called.swap(true, Ordering::SeqCst) {
            callback();
        }
    });
    (registration, called)
}

#[test]
fn reentrant_configuration_keeps_the_newest_complete_profile() {
    let _limits = CacheLimitsGuard::new(INITIAL);
    let (_registration, called) = register_once(|| {
        assert_published(OUTER);
        configure_limits(REENTRANT);
        assert_published(REENTRANT);
    });

    configure_limits(OUTER);

    assert!(called.load(Ordering::SeqCst));
    assert_published(REENTRANT);
}

#[test]
fn zero_capacity_eviction_does_not_overwrite_reentrant_configuration() {
    let _limits = CacheLimitsGuard::new(INITIAL);
    let (_registration, called) = register_once(|| {
        assert_published(CacheLimits {
            capacity: 0,
            ..INITIAL
        });
        configure_limits(REENTRANT);
        assert_published(REENTRANT);
    });

    set_global_capacity(0);

    assert!(called.load(Ordering::SeqCst));
    assert_published(REENTRANT);
}

#[test]
fn configuration_callback_panic_leaves_the_complete_profile_published() {
    let _limits = CacheLimitsGuard::new(INITIAL);
    let (_registration, called) = register_once(|| {
        assert_published(OUTER);
        panic!("configuration callback failed");
    });

    let result = std::panic::catch_unwind(|| configure_limits(OUTER));

    assert_eq!(
        result.expect_err("callback panic").downcast_ref::<&str>(),
        Some(&"configuration callback failed")
    );
    assert!(called.load(Ordering::SeqCst));
    assert_published(OUTER);
    configure_limits(REENTRANT);
    assert_published(REENTRANT);
}

#[test]
fn capacity_callback_panic_leaves_the_complete_profile_published() {
    let _limits = CacheLimitsGuard::new(INITIAL);
    let disabled = CacheLimits {
        capacity: 0,
        ..INITIAL
    };
    let (_registration, called) = register_once(move || {
        assert_published(disabled);
        panic!("capacity callback failed");
    });

    let result = std::panic::catch_unwind(|| set_global_capacity(0));

    assert_eq!(
        result.expect_err("callback panic").downcast_ref::<&str>(),
        Some(&"capacity callback failed")
    );
    assert!(called.load(Ordering::SeqCst));
    assert_published(disabled);
    configure_limits(REENTRANT);
    assert_published(REENTRANT);
}
