//! Original prepaid execution survives detached handoff and scoped installation.

use super::*;
use mv::{BlockMode, PublicationPreparationError, storage::Detached};

type Journal = Detached<Payload, Payload, usize, Prepaid<NativeStoragePolicy>>;

fn capture(
    storage: &NativeStorage,
    budget: &AllocationBudget,
    mode: BlockMode,
    value: u8,
) -> Journal {
    storage
        .try_capture_admitted_block(mode, |block| {
            drop(put(block, budget, 7, value));
            assert!(
                block
                    .try_remove_admitted(removal_key(budget, 99))
                    .unwrap()
                    .is_none()
            );
            Ok::<_, ()>(73)
        })
        .unwrap()
}

fn prepare<'scope, 'target>(
    journal: Journal,
    scope: &'scope mv::allocation::AllocationScope<'scope>,
    target: &'target NativeStorage,
) -> mv::storage::AdmittedPreparedPublication<
    'scope,
    'target,
    Payload,
    Payload,
    usize,
    NativeStoragePolicy,
> {
    journal
        .try_prepare_admitted(scope, target)
        .unwrap_or_else(|(_, error, _)| panic!("original preparation: {error:?}"))
}

#[test]
fn captured_prepaid_successors_detach_abort_and_publish_at_full_capacity_without_copy() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let budget = AllocationBudget::new(8 << 20);
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0x11);
    let old = storage.view();
    let old_pointer = old.get(&7).unwrap().pointer();
    struct Window;
    impl Drop for Window {
        fn drop(&mut self) {
            ALLOCATIONS.with(|count| count.set(None));
        }
    }
    let window = Window;
    let mut held = None;
    let journal = storage
        .try_capture_admitted_block(BlockMode::Ordinary, |block| {
            drop(put(block, &budget, 7, 0x22));
            assert!(put(block, &budget, 8, 0x28).is_none());
            held = Some(
                budget
                    .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
                    .unwrap(),
            );
            ALLOCATIONS.with(|count| assert!(count.replace(Some(0)).is_none()));
            Ok::<_, ()>(73)
        })
        .unwrap();
    let allocations = ALLOCATIONS.with(|count| count.get().unwrap());
    drop(window);
    assert_eq!(allocations, 0, "capture must move the original successors");
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    assert_eq!(storage.view().get(&7).unwrap().pointer(), old_pointer);
    let after = journal
        .touched_entries()
        .next()
        .unwrap()
        .after
        .unwrap()
        .pointer();
    let copies = (counters.keys.load(SeqCst), counters.values.load(SeqCst));
    without_allocations(|| {
        budget.with_deferred_refund_notifications(|scope| {
            let journal = prepare(journal, scope, &storage).abort().0;
            assert_eq!(
                journal
                    .touched_entries()
                    .next()
                    .unwrap()
                    .after
                    .unwrap()
                    .pointer(),
                after
            );
            assert_eq!(*journal.admission(), 73);
            assert_eq!(
                prepare(journal, scope, &storage).publish().into_admission(),
                73
            );
        })
    });
    assert_eq!(
        (counters.keys.load(SeqCst), counters.values.load(SeqCst)),
        copies
    );
    marker(storage.view().get(&7), 0x22);
    marker(old.get(&7), 0x11);
    drop(held);
    assert_eq!(old.get(&7).unwrap().pointer(), old_pointer);
    without_allocations(|| drop(old));
    without_allocations(|| drop(storage));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn captured_prepaid_refusals_return_original_owner_for_exact_retry() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let budget = AllocationBudget::new(8 << 20);
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    let foreign = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0x31);
    let journal = capture(&storage, &budget, BlockMode::Ordinary, 0x32);
    let pointer = journal
        .touched_entries()
        .next()
        .unwrap()
        .after
        .unwrap()
        .pointer();
    let wrong_pool = AllocationBudget::new(budget.limit_bytes());
    let journal = without_allocations(|| {
        wrong_pool.with_deferred_refund_notifications(|scope| {
            match journal.try_prepare_admitted(scope, &storage) {
                Err((
                    journal,
                    PublicationPreparationError::Admission(AdmittedStorageError::ScopeIdentity),
                    _,
                )) => journal,
                _ => panic!("the original pool scope is required"),
            }
        })
    });
    let mut journal = Some(without_allocations(|| {
        budget.with_deferred_refund_notifications(|scope| {
            match journal.try_prepare_admitted(scope, &foreign) {
                Err((journal, PublicationPreparationError::Changed, _)) => journal,
                _ => panic!("equal storage cannot replace the original owner"),
            }
        })
    }));
    let mut release = None;
    storage
        .try_with_admitted_block(|_| {
            budget.with_deferred_refund_notifications(|scope| {
                match journal
                    .take()
                    .unwrap()
                    .try_prepare_admitted(scope, &storage)
                {
                    Err((original, PublicationPreparationError::Busy(wait), _)) => {
                        let mut future = std::pin::pin!(wait.clone().wait_for_release());
                        assert!(
                            future
                                .as_mut()
                                .poll(&mut Context::from_waker(Waker::noop()))
                                .is_pending()
                        );
                        journal = Some(original);
                        release = Some(wait);
                    }
                    _ => panic!("held original writer must defer"),
                }
            });
            Err::<(), _>(())
        })
        .unwrap_err();
    let wait = release.unwrap();
    let mut future = std::pin::pin!(wait.wait_for_release());
    assert!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_ready()
    );
    let journal = journal.unwrap();
    assert_eq!(
        journal
            .touched_entries()
            .next()
            .unwrap()
            .after
            .unwrap()
            .pointer(),
        pointer
    );
    let clone = budget.clone();
    without_allocations(|| {
        clone.with_deferred_refund_notifications(|scope| {
            assert_eq!(
                prepare(journal, scope, &storage).publish().into_admission(),
                73
            );
        })
    });
    marker(storage.view().get(&7), 0x32);
    drop((storage, foreign));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn captured_prepaid_replacement_retains_mode_and_rejects_a_changed_pair() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let budget = AllocationBudget::new(8 << 20);
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0x41);
    storage
        .try_with_admitted_block(|block| {
            drop(put(block, &budget, 7, 0x42));
            Ok::<_, ()>(())
        })
        .unwrap();
    let journal = capture(&storage, &budget, BlockMode::Replace, 0x43);
    assert_eq!(journal.mode(), BlockMode::Replace);
    marker(journal.touched_entries().next().unwrap().before, 0x41);
    marker(storage.view().get(&7), 0x42);
    let journal = without_allocations(|| {
        budget
            .with_deferred_refund_notifications(|scope| prepare(journal, scope, &storage).abort().0)
    });
    assert_eq!(journal.mode(), BlockMode::Replace);
    without_allocations(|| {
        budget.with_deferred_refund_notifications(|scope| {
            prepare(journal, scope, &storage).publish().into_admission()
        })
    });
    marker(storage.view().get(&7), 0x43);
    let stale = capture(&storage, &budget, BlockMode::Ordinary, 0x44);
    let pointer = stale
        .touched_entries()
        .next()
        .unwrap()
        .after
        .unwrap()
        .pointer();
    // Even an unchanged current value publishes a different undo/version pair.
    storage
        .try_with_admitted_block(|_| Ok::<_, ()>(()))
        .unwrap();
    let stale = without_allocations(|| {
        budget.with_deferred_refund_notifications(|scope| {
            match stale.try_prepare_admitted(scope, &storage) {
                Err((original, PublicationPreparationError::Changed, _)) => original,
                _ => panic!("stale original publication cannot be installed"),
            }
        })
    });
    assert_eq!(
        stale
            .touched_entries()
            .next()
            .unwrap()
            .after
            .unwrap()
            .pointer(),
        pointer
    );
    drop(storage);
    marker(stale.touched_entries().next().unwrap().after, 0x44);
    without_allocations(|| drop(stale));
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}

struct ReenterPair {
    first: Arc<NativeStorage>,
    second: Arc<NativeStorage>,
    wakes: AtomicUsize,
    poisoned: bool,
}
impl Wake for ReenterPair {
    fn wake(self: Arc<Self>) {
        for storage in [&self.first, &self.second] {
            let result = storage.try_with_admitted_block(|_| Err::<(), _>(()));
            if self.poisoned {
                assert!(
                    matches!(
                        result,
                        Err(AdmittedBlockError::Admission(
                            AdmittedStorageError::Poisoned { .. }
                        ))
                    ),
                    "unwound physical writers must already be released and poisoned"
                );
            } else {
                assert!(
                    matches!(result, Err(AdmittedBlockError::Callback(()))),
                    "both original pairs must be unlocked"
                );
            }
        }
        self.wakes.fetch_add(1, SeqCst);
    }
}

#[test]
fn captured_prepaid_pair_shares_one_scope_through_publication_abort_and_unwind() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    for mode in 0..4 {
        let unwind = mode == 1 || mode == 2;
        reset();
        let counters = Arc::new(Counters::default());
        let _context = PolicyContext::new(&counters);
        let budget = AllocationBudget::new(8 << 20);
        let first = Arc::new(NativeStorage::try_new_admitted(budget.clone()).unwrap());
        let second = Arc::new(NativeStorage::try_new_admitted(budget.clone()).unwrap());
        seed(&first, &budget, 0x51);
        seed(&second, &budget, 0x61);
        let first_before = first.view();
        let second_before = second.view();
        let a = capture(&first, &budget, BlockMode::Ordinary, 0x52);
        let b = capture(&second, &budget, BlockMode::Ordinary, 0x62);
        let wake = Arc::new(ReenterPair {
            first: Arc::clone(&first),
            second: Arc::clone(&second),
            wakes: AtomicUsize::new(0),
            poisoned: mode == 2,
        });
        let waker = Waker::from(Arc::clone(&wake));
        let mut context = Context::from_waker(&waker);
        let held = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
            .unwrap();
        let AllocationRefusal::Capacity { release, .. } = budget.try_reserve_bytes(1).unwrap_err()
        else {
            panic!("pool full");
        };
        let mut future = std::pin::pin!(release.wait_for_release());
        assert!(future.as_mut().poll(&mut context).is_pending());
        let result = catch_unwind(AssertUnwindSafe(|| {
            budget.with_deferred_refund_notifications(|scope| {
                let a = prepare(a, scope, &first);
                let b = prepare(b, scope, &second);
                drop(held);
                assert_eq!(wake.wakes.load(SeqCst), 0);
                if mode == 2 {
                    panic!("aggregate interrupted with both physical pairs held");
                }
                if mode == 3 {
                    without_allocations(|| drop((a, b)));
                    assert_eq!(wake.wakes.load(SeqCst), 0);
                    return;
                }
                if unwind {
                    // Explicit abort keeps both physical locks healthy before an
                    // unrelated caller unwind; both original journals remain owned.
                    let _a = a.abort();
                    let _b = b.abort();
                    panic!("aggregate caller interrupted");
                }
                let a = a.publish();
                assert_eq!(wake.wakes.load(SeqCst), 0);
                let b = b.publish();
                assert_eq!(wake.wakes.load(SeqCst), 0);
                assert_eq!(a.into_admission(), 73);
                assert_eq!(b.into_admission(), 73);
            })
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(wake.wakes.load(SeqCst), 1);
        assert!(future.as_mut().poll(&mut context).is_ready());
        if mode == 2 {
            marker(first_before.get(&7), 0x51);
            marker(second_before.get(&7), 0x61);
            assert!(catch_unwind(AssertUnwindSafe(|| first.view())).is_err());
            assert!(catch_unwind(AssertUnwindSafe(|| second.view())).is_err());
        } else {
            marker(first.view().get(&7), if mode != 0 { 0x51 } else { 0x52 });
            marker(second.view().get(&7), if mode != 0 { 0x61 } else { 0x62 });
        }
        drop((first_before, second_before));
        drop((waker, wake, first, second));
        reclaimed_since(0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn captured_prepaid_caught_edit_panic_cannot_escape_as_a_journal() {
    let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    reset();
    let counters = Arc::new(Counters::default());
    let _context = PolicyContext::new(&counters);
    let budget = AllocationBudget::new(8 << 20);
    let storage = NativeStorage::try_new_admitted(budget.clone()).unwrap();
    seed(&storage, &budget, 0x71);
    let original = storage.view();
    let result = catch_unwind(AssertUnwindSafe(|| {
        storage.try_capture_admitted_block(BlockMode::Ordinary, |block| {
            FACTORY_FAULT.with(|mode| mode.set(3));
            let caught = catch_unwind(AssertUnwindSafe(|| drop(put(block, &budget, 7, 0x72))));
            FACTORY_FAULT.with(|mode| mode.set(0));
            assert!(caught.is_err());
            Ok::<_, ()>(73)
        })
    }));
    assert!(result.is_err());
    marker(original.get(&7), 0x71);
    marker(storage.view().get(&7), 0x71);
    assert!(matches!(
        storage.try_capture_admitted_block(BlockMode::Ordinary, |_| Ok::<_, ()>(73)),
        Err(AdmittedBlockError::Admission(
            AdmittedStorageError::Poisoned { .. }
        ))
    ));
    drop(original);
    drop(storage);
    reclaimed_since(0);
    assert_eq!(budget.reserved_bytes(), 0);
}
