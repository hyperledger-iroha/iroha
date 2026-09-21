//! Publication identity contention must not block detached preparation.

use super::*;

#[test]
fn cell_identity_contention_retains_original_journal_without_admission() {
    let target = crate::cell::Cell::new(String::from("before"));
    let mut block = target.block();
    *block = String::from("after");
    let journal = block
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("detach original journal"));
    let original = journal.touched_value().unwrap().after.as_ptr();
    let identity = target.publication.lock_version();
    let (journal, error) = journal
        .try_prepare_publication(&target, |_, _| -> Result<(), ()> {
            panic!("busy identity observation must precede admission")
        })
        .err()
        .expect("identity lock is busy");
    assert!(matches!(error, PublicationPreparationError::Busy(_)));
    assert_eq!(journal.touched_value().unwrap().after.as_ptr(), original);
    drop(identity);
    journal
        .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("retry same cell journal"))
        .publish();
    assert_eq!(&**target.view(), "after");
}

#[test]
fn storage_identity_contention_retains_original_journal_without_admission() {
    let target: crate::storage::Storage<u64, String> =
        [(1, String::from("before"))].into_iter().collect();
    let mut block = target.block();
    block.insert(1, String::from("after"));
    let journal = block
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("detach original journal"));
    let original = journal
        .touched_entries()
        .next()
        .unwrap()
        .after
        .unwrap()
        .as_ptr();
    let identity = target.publication.lock_version();
    let (journal, error) = journal
        .try_prepare_publication(&target, |_, _| -> Result<(), ()> {
            panic!("busy identity observation must precede admission")
        })
        .err()
        .expect("identity lock is busy");
    assert!(matches!(error, PublicationPreparationError::Busy(_)));
    assert_eq!(
        journal
            .touched_entries()
            .next()
            .unwrap()
            .after
            .unwrap()
            .as_ptr(),
        original
    );
    drop(identity);
    journal
        .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("retry same storage journal"))
        .publish();
    assert_eq!(target.view().get(&1).map(String::as_str), Some("after"));
}

struct CellIdentityAdmission<'a> {
    target: &'a crate::cell::Cell<String>,
    _identity: crate::ReleaseGuard<'a, std::sync::MutexGuard<'a, Identity<Version>>>,
}
impl Drop for CellIdentityAdmission<'_> {
    fn drop(&mut self) {
        assert!(
            self.target.revert.try_write().is_some(),
            "undo writer precedes admission release"
        );
        assert!(
            self.target.blocks.try_write().is_some(),
            "current writer precedes admission release"
        );
    }
}

#[test]
fn cell_identity_contention_after_admission_releases_both_writers_before_guard() {
    let target = crate::cell::Cell::new(String::from("before"));
    let mut block = target.block();
    *block = String::from("after");
    let journal = block
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("detach original journal"));
    let original = journal.touched_value().unwrap().after.as_ptr();
    let (journal, error) = journal
        .try_prepare_publication(&target, |_, _| {
            Ok::<_, ()>(CellIdentityAdmission {
                target: &target,
                _identity: target.publication.lock_version(),
            })
        })
        .err()
        .expect("identity changed to busy after admission");
    assert!(matches!(error, PublicationPreparationError::Busy(_)));
    assert_eq!(journal.touched_value().unwrap().after.as_ptr(), original);
    assert_eq!(&**target.view(), "before");
    assert!(journal.matches_current(&target));
    journal
        .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("retry after identity contention"))
        .publish();
}

struct StorageIdentityAdmission<'a> {
    target: &'a crate::storage::Storage<u64, String>,
    _identity: crate::ReleaseGuard<'a, std::sync::MutexGuard<'a, Identity<Version>>>,
}
impl Drop for StorageIdentityAdmission<'_> {
    fn drop(&mut self) {
        assert!(self.target.revert.try_write().is_some());
        assert!(self.target.blocks.try_write().is_some());
    }
}

#[test]
fn storage_identity_contention_after_admission_releases_both_writers_before_guard() {
    let target: crate::storage::Storage<u64, String> =
        [(1, String::from("before"))].into_iter().collect();
    let mut block = target.block();
    block.insert(1, String::from("after"));
    let journal = block
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("detach original journal"));
    let original = journal
        .touched_entries()
        .next()
        .unwrap()
        .after
        .unwrap()
        .as_ptr();
    let (journal, error) = journal
        .try_prepare_publication(&target, |_, _| {
            Ok::<_, ()>(StorageIdentityAdmission {
                target: &target,
                _identity: target.publication.lock_version(),
            })
        })
        .err()
        .expect("identity changed to busy after admission");
    assert!(matches!(error, PublicationPreparationError::Busy(_)));
    assert_eq!(
        journal
            .touched_entries()
            .next()
            .unwrap()
            .after
            .unwrap()
            .as_ptr(),
        original
    );
    assert_eq!(target.view().get(&1).map(String::as_str), Some("before"));
    assert!(journal.matches_current(&target));
    journal
        .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("retry after identity contention"))
        .publish();
}

#[test]
fn poisoned_publication_is_a_local_failure_instead_of_endless_busy_retry() {
    let target = crate::cell::Cell::new(String::from("before"));
    let mut block = target.block();
    *block = String::from("after");
    let journal = block
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("detach original journal"));
    let original = journal.touched_value().unwrap().after.as_ptr();
    let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _identity = target.publication.lock_version();
        panic!("simulate failed joint publication");
    }));
    assert!(poisoned.is_err());
    let (journal, error) = journal
        .try_prepare_publication(&target, |_, _| -> Result<(), ()> {
            panic!("poisoned publication must not acquire installation resources")
        })
        .err()
        .expect("poisoned identity");
    assert_eq!(error, PublicationPreparationError::Poisoned);
    assert_eq!(journal.touched_value().unwrap().after.as_ptr(), original);
    assert_eq!(&**target.view(), "before");
}

#[test]
fn busy_identity_wait_is_signaled_after_the_actual_metadata_guard_releases() {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Waker},
    };
    let target = crate::cell::Cell::new(10_u64);
    let journal = target.block().try_detach(|_| Ok::<_, ()>(())).unwrap();
    let identity = target.publication.lock_version();
    let (journal, error) = journal
        .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
        .err()
        .expect("metadata busy");
    let PublicationPreparationError::Busy(wait) = error else {
        panic!("metadata wait");
    };
    let mut wait = wait.wait_for_release();
    let mut context = Context::from_waker(Waker::noop());
    assert!(Pin::new(&mut wait).poll(&mut context).is_pending());
    drop(identity);
    assert!(Pin::new(&mut wait).poll(&mut context).is_ready());
    assert!(
        journal
            .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
            .is_ok()
    );
}

#[test]
fn funded_identity_refund_observes_unlocked_publication_even_on_release_unwind() {
    use crate::allocation::{AllocationBudget, AllocationRefusal};
    use std::{
        future::Future,
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering::SeqCst},
        },
        task::{Context, Wake, Waker},
    };
    struct Probe {
        owner: Arc<super::Publication>,
        wakes: AtomicUsize,
    }
    impl Wake for Probe {
        fn wake(self: Arc<Self>) {
            assert!(
                !matches!(
                    self.owner.version.try_lock(),
                    Err(std::sync::TryLockError::WouldBlock)
                ),
                "identity refund ran under the publication lock"
            );
            self.wakes.fetch_add(1, SeqCst);
        }
    }
    for unwind in [false, true] {
        let initial = super::Publication::allocation_demand().unwrap().bytes();
        let successor = super::NextPublication::allocation_demand().unwrap().bytes();
        let budget = AllocationBudget::new(initial + successor);
        let owner = Arc::new(super::Publication::from_admission(
            budget.try_reserve_bytes(initial).unwrap(),
        ));
        let next =
            super::NextPublication::from_admission(budget.try_reserve_bytes(successor).unwrap());
        let probe = Arc::new(Probe {
            owner: Arc::clone(&owner),
            wakes: AtomicUsize::new(0),
        });
        let waker = Waker::from(Arc::clone(&probe));
        let mut context = Context::from_waker(&waker);
        let AllocationRefusal::Capacity { release, .. } = budget.try_reserve_bytes(1).unwrap_err()
        else {
            panic!("original identity pool must be full");
        };
        let mut wait = std::pin::pin!(release.wait_for_release());
        assert!(wait.as_mut().poll(&mut context).is_pending());
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            owner.publish_retaining(
                next,
                || (),
                |()| {
                    assert_eq!(probe.wakes.load(SeqCst), 0);
                    assert!(!unwind, "release interruption");
                },
            )
        }));
        assert_eq!(outcome.is_err(), unwind);
        assert_eq!(probe.wakes.load(SeqCst), 1);
        assert!(wait.as_mut().poll(&mut context).is_ready());
        assert_eq!(budget.reserved_bytes(), initial);
        drop((waker, probe, owner));
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
