//! Original funded-runtime releases notify only after every cache guard has retired.

use super::*;
use iroha_allocation::{AllocationBudget, AllocationRefusal, release::ReleaseFuture};
use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Wake, Waker},
};

struct ReenterCache {
    cache: PreparedContractCache,
    enclosing: Option<Arc<Mutex<()>>>,
    calls: AtomicUsize,
    locked: AtomicBool,
    reentered: AtomicUsize,
}

impl Wake for ReenterCache {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        // Record failure without blocking or panicking from an unwind callback.
        // A regressed direct wake must fail the assertion instead of hanging.
        let Some(cache_guard) = self.cache.inner.try_lock() else {
            self.locked.store(true, Ordering::SeqCst);
            return;
        };
        drop(cache_guard);
        if let Some(enclosing) = &self.enclosing {
            let Some(guard) = enclosing.try_lock() else {
                self.locked.store(true, Ordering::SeqCst);
                return;
            };
            drop(guard);
        }
        // Reenter the real cache API from the synchronous original-pool wake.
        std::hint::black_box(self.cache.stats());
        self.reentered.fetch_add(1, Ordering::SeqCst);
    }
}

fn enabled_retention() -> ivm::ivm_cache::CacheLimitsGuard {
    ivm::ivm_cache::CacheLimitsGuard::new(ivm::ivm_cache::CacheLimits {
        capacity: 4,
        max_bytes: usize::MAX,
        max_decoded_ops: 0,
    })
}

fn idle_funded_runtime() -> (PreparedContractCache, AllocationBudget) {
    let budget = AllocationBudget::new(64 * 1024 * 1024);
    let cache = PreparedContractCache::with_execution_budget(1, budget.clone());
    let program = super::tests::minimal_program();
    let prepared = cache
        .get_or_prepare(ivm::contract_code_hash(&program), &program)
        .unwrap();
    let runtime = cache
        .checkout_runtime(&prepared, 10_000, ivm::Memory::HEAP_MAX_SIZE)
        .unwrap();
    let funded_bytes = budget.reserved_bytes();
    assert!(funded_bytes > 0);
    drop(runtime);
    assert_eq!(budget.reserved_bytes(), funded_bytes);
    assert_eq!(cache.stats().runtime_dirty_resets, 1);
    assert!(cache.with_store(|store| {
        store
            .nested_runtimes
            .values()
            .any(|pool| !pool.available.is_empty())
    }));
    (cache, budget)
}

fn observe_capacity<'a>(
    cache: &PreparedContractCache,
    budget: &AllocationBudget,
    enclosing: Option<Arc<Mutex<()>>>,
    registration: &'a mut iroha_allocation::release::ReleaseRegistration,
) -> (ReleaseFuture<'a>, Arc<ReenterCache>) {
    let AllocationRefusal::Capacity { release, .. } = budget.try_reserve_bytes(1).unwrap_err()
    else {
        panic!("the real original pool must be occupied by the idle runtime");
    };
    let mut wait = release.wait_for_release(registration);
    let probe = Arc::new(ReenterCache {
        cache: cache.clone(),
        enclosing,
        calls: AtomicUsize::new(0),
        locked: AtomicBool::new(false),
        reentered: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&probe));
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    (wait, probe)
}

fn assert_reentered(mut wait: ReleaseFuture<'_>, probe: &Arc<ReenterCache>) {
    assert!(
        !probe.locked.load(Ordering::SeqCst),
        "refund callback observed a held physical guard"
    );
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    assert_eq!(probe.reentered.load(Ordering::SeqCst), 1);
    let waker = Waker::from(Arc::clone(probe));
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_ready()
    );
}

#[test]
fn prepared_lru_eviction_refund_reenters_cache_after_unlock() {
    let _limits = enabled_retention();
    let (cache, budget) = idle_funded_runtime();
    let mut registration = crate::unit_test_support::release_registration(&budget);
    let waiter_bytes = iroha_allocation::release::ReleaseRegistration::allocation_layout().size();
    // Admission itself now owns instruction/image backing. Admit the incoming
    // artifact while capacity exists, then exercise only its LRU publication
    // under full original-pool pressure.
    let before = budget.reserved_bytes();
    let mut next = super::tests::minimal_program();
    next[8..16].copy_from_slice(&23_u64.to_le_bytes());
    let next = ivm::prepare_contract_with_memory_budget(&next, &budget).unwrap();
    let next_bytes = budget.reserved_bytes() - before;
    budget.set_limit_bytes(budget.reserved_bytes());
    let (wait, probe) = observe_capacity(&cache, &budget, None, &mut registration);
    cache.publish(next).unwrap();
    assert_eq!(budget.reserved_bytes(), next_bytes + waiter_bytes);
    assert_reentered(wait, &probe);
    drop(
        budget
            .try_reserve_bytes(1)
            .expect("released original credit is usable"),
    );
    assert_eq!(cache.stats().evictions, 1);
    cache.with_store(|store| store.clear_storage());
    assert_eq!(budget.reserved_bytes(), waiter_bytes);
    drop(registration);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn configured_eviction_keeps_original_pool_and_enclosing_refund_batch() {
    let _limits = enabled_retention();
    let (cache, budget) = idle_funded_runtime();
    let mut registration = crate::unit_test_support::release_registration(&budget);
    let waiter_bytes = iroha_allocation::release::ReleaseRegistration::allocation_layout().size();
    let retained = budget.reserved_bytes();
    // Shrink the same pool below its retained runtime; no replacement pool may
    // forgive those charges or supply a different release observation.
    budget.set_limit_bytes(waiter_bytes + 1);
    assert_eq!(budget.reserved_bytes(), retained);
    let enclosing = Arc::new(Mutex::new(()));
    let (wait, probe) = observe_capacity(
        &cache,
        &budget,
        Some(Arc::clone(&enclosing)),
        &mut registration,
    );
    let mut refunds = budget.deferred_refund_batch();
    let guard = enclosing.lock();
    refunds.with_scope(|_| {
        ivm::ivm_cache::configure_limits(ivm::ivm_cache::CacheLimits {
            capacity: 0,
            max_bytes: 0,
            max_decoded_ops: 0,
        });
        assert_eq!(budget.reserved_bytes(), waiter_bytes);
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
        drop(
            budget
                .try_reserve_bytes(1)
                .expect("refund credit precedes notifications"),
        );
    });
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    drop(guard);
    drop(refunds);
    assert_reentered(wait, &probe);
    assert!(cache.execution_budget().same_pool(&budget));
    assert_eq!(cache.execution_budget().limit_bytes(), waiter_bytes + 1);
    assert_eq!(budget.reserved_bytes(), waiter_bytes);
    drop(registration);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn prepared_store_unwind_unlocks_before_refund_callback_and_preserves_original_panic() {
    let _limits = enabled_retention();
    let (cache, budget) = idle_funded_runtime();
    let mut registration = crate::unit_test_support::release_registration(&budget);
    let waiter_bytes = iroha_allocation::release::ReleaseRegistration::allocation_layout().size();
    budget.set_limit_bytes(budget.reserved_bytes());
    let (wait, probe) = observe_capacity(&cache, &budget, None, &mut registration);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cache.with_store(|store| {
            store.clear_storage();
            assert_eq!(budget.reserved_bytes(), waiter_bytes);
            assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
            panic!("abandon prepared cache operation");
        });
    }));
    let panic = result.expect_err("the original operation must still unwind");
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"abandon prepared cache operation")
    );
    assert_reentered(wait, &probe);
    assert_eq!(budget.reserved_bytes(), waiter_bytes);
    assert!(cache.with_store(|store| store.entries.is_empty()));
    drop(registration);
    assert_eq!(budget.reserved_bytes(), 0);
}
