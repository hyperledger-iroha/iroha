//! Original-pool histogram admission, shared cache borrowers and unlocked refunds.

use super::*;
use iroha_allocation::{AllocationBudget, AllocationRefusal};
use std::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Wake, Waker},
};

const LIMIT: usize = 64 * 1024 * 1024;

fn retention(enabled: bool) -> ivm::ivm_cache::CacheLimitsGuard {
    ivm::ivm_cache::CacheLimitsGuard::new(ivm::ivm_cache::CacheLimits {
        capacity: usize::from(enabled) * 4,
        max_bytes: if enabled { LIMIT } else { 0 },
        max_decoded_ops: 0,
    })
}

fn cache(budget: &AllocationBudget) -> IvmCache {
    IvmCache::with_prepared_contract_cache(
        2,
        PreparedContractCache::with_execution_budget(2, budget.clone()),
    )
}

fn with_syscalls(mut program: Vec<u8>) -> Vec<u8> {
    // Whole-program analysis includes valid unreachable instructions too.
    for number in [
        ivm::syscalls::SYSCALL_ALLOC,
        ivm::syscalls::SYSCALL_DEBUG_PRINT,
        ivm::syscalls::SYSCALL_ALLOC,
    ] {
        program.extend_from_slice(&ivm::encoding::wide::encode_syscallx(number).to_le_bytes());
    }
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    program
}

fn assert_histogram(analysis: &ProgramAnalysis) {
    assert_eq!(
        &*analysis.syscalls,
        &[
            ivm::analysis::SyscallUsage {
                number: ivm::syscalls::SYSCALL_DEBUG_PRINT,
                count: 1
            },
            ivm::analysis::SyscallUsage {
                number: ivm::syscalls::SYSCALL_ALLOC,
                count: 2
            },
        ]
    );
}

#[test]
fn contract_analysis_refuses_original_pool_then_shares_cached_result_until_final_borrower() {
    let _retention = retention(true);
    let budget = AllocationBudget::new(LIMIT);
    let mut cache = cache(&budget);
    let program = with_syscalls(super::tests::minimal_program());
    let summary = cache.summarize_program(&program).unwrap();
    let baseline = budget.reserved_bytes();
    budget.set_limit_bytes(0);
    let failure = cache.analyze_program(&summary, &program).unwrap_err();
    assert!(matches!(
        failure.into_vm_error(),
        ivm::VMError::AllocationDeferred(AllocationRefusal::ExceedsLimit { limit_bytes: 0, .. })
    ));
    assert_eq!(budget.reserved_bytes(), baseline);
    assert!(cache.with_local(|local| local.analyses.is_empty()));
    budget.set_limit_bytes(LIMIT);
    let occupied = budget.try_reserve_bytes(LIMIT - baseline).unwrap();
    assert!(matches!(
        cache
            .analyze_program(&summary, &program)
            .unwrap_err()
            .into_vm_error(),
        ivm::VMError::AllocationDeferred(AllocationRefusal::Capacity { .. })
    ));
    assert!(cache.with_local(|local| local.analyses.is_empty()));
    drop(occupied);
    let first = cache.analyze_program(&summary, &program).unwrap();
    assert_histogram(&first);
    let charged = budget.reserved_bytes();
    assert!(charged > baseline);
    budget.set_limit_bytes(0);
    let second = cache.analyze_program(&summary, &program).unwrap();
    assert_eq!(first.syscalls.as_ptr(), second.syscalls.as_ptr());
    assert_eq!(budget.reserved_bytes(), charged);
    cache.with_local(LocalCacheStore::clear_storage);
    drop(first);
    assert_eq!(
        budget.reserved_bytes(),
        charged,
        "evicted output remains borrowed"
    );
    drop(second);
    assert_eq!(budget.reserved_bytes(), baseline);
    drop((cache, summary));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn generic_analysis_retries_original_refusal_without_retention_or_output_copy() {
    let _retention = retention(false);
    let budget = AllocationBudget::new(LIMIT);
    let mut cache = cache(&budget);
    let program = with_syscalls(super::tests::minimal_generic_program());
    let summary = cache.summarize_generic_program(&program).unwrap();
    let baseline = budget.reserved_bytes();
    budget.set_limit_bytes(0);
    assert!(matches!(
        cache
            .analyze_generic_program(&summary)
            .unwrap_err()
            .into_vm_error(),
        ivm::VMError::AllocationDeferred(AllocationRefusal::ExceedsLimit { .. })
    ));
    assert_eq!(budget.reserved_bytes(), baseline);
    budget.set_limit_bytes(LIMIT);
    let first = cache.analyze_generic_program(&summary).unwrap();
    let second = cache.analyze_generic_program(&summary).unwrap();
    assert_histogram(&first);
    assert_eq!(first.syscalls, second.syscalls);
    assert_ne!(first.syscalls.as_ptr(), second.syscalls.as_ptr());
    assert!(cache.with_local(|local| local.analyses.is_empty()));
    let borrowed = first.clone();
    let charged = budget.reserved_bytes();
    drop(first);
    assert_eq!(budget.reserved_bytes(), charged);
    drop(second);
    assert!(budget.reserved_bytes() > baseline);
    drop((cache, summary));
    assert_histogram(&borrowed);
    assert!(budget.reserved_bytes() > 0);
    drop(borrowed);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn foreign_summary_cannot_supply_execution_credit_to_another_states_analysis() {
    let _retention = retention(true);
    let first_budget = AllocationBudget::new(LIMIT);
    let mut first = cache(&first_budget);
    let program = with_syscalls(super::tests::minimal_program());
    let summary = first.summarize_program(&program).unwrap();
    let original = first_budget.reserved_bytes();
    let second_budget = AllocationBudget::new(0);
    let mut second = cache(&second_budget);
    assert!(matches!(
        second
            .analyze_program(&summary, &program)
            .unwrap_err()
            .into_vm_error(),
        ivm::VMError::AllocationDeferred(AllocationRefusal::ExceedsLimit { limit_bytes: 0, .. })
    ));
    assert_eq!(first_budget.reserved_bytes(), original);
    assert_eq!(second_budget.peak_reserved_bytes(), 0);
    second_budget.set_limit_bytes(LIMIT);
    let analysis = second.analyze_program(&summary, &program).unwrap();
    assert_histogram(&analysis);
    let charged = second_budget.reserved_bytes();
    assert!(charged > 0);
    drop((first, summary));
    assert_eq!(first_budget.reserved_bytes(), 0);
    drop(second);
    assert_eq!(second_budget.reserved_bytes(), charged);
    drop(analysis);
    assert_eq!(second_budget.reserved_bytes(), 0);
}

struct ReenterAnalysis {
    outer: Arc<Mutex<IvmCache>>,
    local: Arc<Mutex<LocalCacheStore>>,
    calls: AtomicUsize,
    locked: AtomicBool,
}
impl Wake for ReenterAnalysis {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let Some(local) = self.local.try_lock() else {
            self.locked.store(true, Ordering::SeqCst);
            return;
        };
        drop(local);
        let Some(outer) = self.outer.try_lock() else {
            self.locked.store(true, Ordering::SeqCst);
            return;
        };
        drop(outer);
        std::hint::black_box(IvmCache::with_locked(&self.outer, |cache| cache.stats()));
    }
}

#[test]
fn histogram_final_refund_reenters_after_local_and_state_guards_on_success_and_unwind() {
    let _retention = retention(true);
    for unwind in [false, true] {
        let budget = AllocationBudget::new(LIMIT);
        let mut registration = crate::unit_test_support::release_registration(&budget);
        let mut cache = cache(&budget);
        let program = with_syscalls(super::tests::minimal_program());
        let summary = cache.summarize_program(&program).unwrap();
        let baseline = budget.reserved_bytes();
        drop(cache.analyze_program(&summary, &program).unwrap());
        assert!(budget.reserved_bytes() > baseline);
        budget.set_limit_bytes(budget.reserved_bytes());
        let local = Arc::clone(&cache.local);
        let observer = Arc::new(ReenterAnalysis {
            outer: Arc::new(Mutex::new(cache)),
            local,
            calls: AtomicUsize::new(0),
            locked: AtomicBool::new(false),
        });
        let AllocationRefusal::Capacity { release, .. } = budget.try_reserve_bytes(1).unwrap_err()
        else {
            panic!("histogram occupies its original pool");
        };
        let mut wait = release.wait_for_release(&mut registration);
        let waker = Waker::from(Arc::clone(&observer));
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(&waker))
                .is_pending()
        );
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            IvmCache::with_locked(&observer.outer, |cache| {
                cache.with_local(|local| {
                    local.analyses.clear();
                    assert_eq!(budget.reserved_bytes(), baseline);
                    assert_eq!(observer.calls.load(Ordering::SeqCst), 0);
                    assert!(!unwind, "analysis cache operation failed");
                });
            });
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(observer.calls.load(Ordering::SeqCst), 1);
        assert!(!observer.locked.load(Ordering::SeqCst));
        assert!(
            Pin::new(&mut wait)
                .poll(&mut Context::from_waker(&waker))
                .is_ready()
        );
        drop((wait, waker, observer, summary));
        assert_eq!(
            budget.reserved_bytes(),
            iroha_allocation::release::ReleaseRegistration::allocation_layout().size()
        );
        drop(registration);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
