//! Real finite Storage acquisition remains with the caller through refusal.

use super::*;
use crate::{BlockAcquisition as _, BlockCapture as _};

struct PairProbe {
    first: Arc<StartStorage>,
    second: Arc<StartStorage>,
    wakes: AtomicUsize,
    busy: AtomicUsize,
}
impl PairProbe {
    fn any_busy(&self) -> bool {
        [
            self.first.revert.try_acquire_writer().is_none(),
            self.first.blocks.try_acquire_writer().is_none(),
            self.second.revert.try_acquire_writer().is_none(),
            self.second.blocks.try_acquire_writer().is_none(),
        ]
        .into_iter()
        .any(|busy| busy)
    }
}
impl Wake for PairProbe {
    fn wake(self: Arc<Self>) {
        self.wakes.fetch_add(1, SeqCst);
        if self.any_busy() {
            self.busy.fetch_add(1, SeqCst);
        }
    }
}
fn watch(source: &ReleaseNotification, probe: &Arc<PairProbe>) -> concread::release::ReleaseFuture {
    let mut future = source.observe().wait_for_release();
    let waker = Waker::from(Arc::clone(probe));
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    future
}

#[test]
fn admitted_scoped_acquisition_rejects_foreign_scope_before_locks_or_policy() {
    let budget = AllocationBudget::new(1 << 20);
    let other = AllocationBudget::new(1 << 20);
    let target = fixture(&budget);
    let before = budget.reserved_bytes();
    let _healthy = StartFault::set(0);
    other.with_deferred_refund_notifications(|scope| {
        assert!(matches!(
            target.try_block_acquisition_admitted(scope),
            Err(AdmittedStorageError::ScopeIdentity)
        ));
        assert_eq!(START_CALLS.with(Cell::get), 0);
        assert!(target.revert.try_acquire_writer().is_some());
        assert!(target.blocks.try_acquire_writer().is_some());
    });
    assert_eq!(budget.reserved_bytes(), before);
    assert_eq!(other.reserved_bytes(), 0);
    drop(target);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn admitted_scoped_start_and_reset_refusals_retain_original_owners_for_release() {
    let budget = AllocationBudget::new(1 << 20);
    let target = fixture(&budget);
    let predecessor = target.publication.capture();
    let before = budget.reserved_bytes();
    let start = StartStorage::writer_start_allocation_demand()
        .unwrap()
        .bytes();
    for allowed in [0, start] {
        budget.with_deferred_refund_notifications(|scope| {
            let held = budget
                .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes() - allowed)
                .unwrap();
            let probe = Probe::new(&target);
            let (_, _, mut undo_future, mut current_future) = register(&target, &probe);
            let _healthy = StartFault::set(0);
            let mut slot = target.try_block_acquisition_admitted(scope).unwrap();
            assert!(matches!(
                slot.try_initialize(BlockMode::Ordinary),
                Err(AdmittedStorageError::Allocation(_))
            ));
            assert_eq!(probe.calls.load(SeqCst), 0);
            if allowed == 0 {
                assert_eq!(START_CALLS.with(Cell::get), 0);
                assert!(target.revert.try_acquire_writer().is_some());
                assert!(target.blocks.try_acquire_writer().is_some());
            } else {
                // Writer admission succeeded; undo reset refused. Both actual
                // writers and the next identity remain in the caller's slot.
                assert_eq!(START_CALLS.with(Cell::get), 2);
                assert!(target.revert.try_acquire_writer().is_none());
                assert!(target.blocks.try_acquire_writer().is_none());
                assert!(budget.reserved_bytes() > before + held.remaining_bytes());
            }
            slot.release();
            slot.release();
            assert!(target.revert.try_acquire_writer().is_some());
            assert!(target.blocks.try_acquire_writer().is_some());
            assert_eq!(probe.calls.load(SeqCst), 0);
            assert!(!ready(&mut undo_future, &probe) && !ready(&mut current_future, &probe));
            assert!(
                catch_unwind(AssertUnwindSafe(|| slot.try_initialize(BlockMode::Ordinary)))
                    .is_err()
            );
            drop(slot);
            assert_eq!(probe.calls.load(SeqCst), if allowed == 0 { 0 } else { 2 });
            assert_eq!(probe.unavailable.load(SeqCst), 0);
            drop((held, undo_future, current_future));
        });
        assert_eq!(budget.reserved_bytes(), before);
        assert_original(&target, &predecessor);
    }
    // A refused slot is terminal; a fresh acquisition retries the same target.
    budget.with_deferred_refund_notifications(|scope| {
        let mut slot = target.try_block_acquisition_admitted(scope).unwrap();
        slot.try_initialize(BlockMode::Ordinary).unwrap();
        let mut block = slot.into_block();
        block.try_insert_admitted(7, 72).unwrap();
        drop(block);
    });
    assert_original(&target, &predecessor);
    drop((predecessor, target));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn admitted_scoped_second_provider_failure_defers_all_aggregate_native_wakes() {
    for mode in [1, 2] {
        let budget = AllocationBudget::new(1 << 20);
        let first = fixture(&budget);
        let second = fixture(&budget);
        let first_predecessor = first.publication.capture();
        let second_predecessor = second.publication.capture();
        let before = budget.reserved_bytes();
        let probe = Arc::new(PairProbe {
            first: Arc::clone(&first),
            second: Arc::clone(&second),
            wakes: AtomicUsize::new(0),
            busy: AtomicUsize::new(0),
        });
        budget.with_deferred_refund_notifications(|scope| {
            let mut first_slot = first.try_block_acquisition_admitted(scope).unwrap();
            let mut second_slot = second.try_block_acquisition_admitted(scope).unwrap();
            let _healthy = StartFault::set(0);
            first_slot.try_initialize(BlockMode::Ordinary).unwrap();
            let waits = [
                watch(&first.revert_released, &probe),
                watch(&first.blocks_released, &probe),
                watch(&second.revert_released, &probe),
                watch(&second.blocks_released, &probe),
            ];
            let fault = StartFault::set(mode);
            let result = catch_unwind(AssertUnwindSafe(|| {
                second_slot.try_initialize(BlockMode::Ordinary)
            }));
            match (mode, result) {
                (
                    1,
                    Ok(Err(AdmittedStorageError::PolicyDemand {
                        expected_bytes,
                        remaining_bytes,
                    })),
                ) => {
                    assert_eq!(expected_bytes, remaining_bytes + 1);
                    assert!(second.blocks.try_acquire_writer().is_none());
                }
                (2, Err(_)) => {
                    // The failing raw current guard unwound, but the first
                    // converted undo and the complete sibling remain retained.
                    assert!(second.blocks.try_acquire_writer().is_some());
                    assert!(second.blocks.is_poisoned());
                }
                (_, result) => panic!("unexpected scoped provider outcome: {result:?}"),
            }
            drop(fault);
            assert!(second.revert.try_acquire_writer().is_none());
            assert!(first.revert.try_acquire_writer().is_none());
            assert!(first.blocks.try_acquire_writer().is_none());
            assert_eq!(probe.wakes.load(SeqCst), 0);
            first_slot.release();
            second_slot.release();
            assert_eq!(probe.wakes.load(SeqCst), 0);
            assert!(!probe.any_busy());
            drop((first_slot, second_slot));
            assert_eq!(probe.wakes.load(SeqCst), 4);
            assert_eq!(probe.busy.load(SeqCst), 0);
            drop(waits);
        });
        assert_eq!(budget.reserved_bytes(), before);
        assert_original(&first, &first_predecessor);
        assert_original(&second, &second_predecessor);
        drop((first_predecessor, second_predecessor, probe, first, second));
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn admitted_scoped_busy_current_retains_undo_until_caller_release() {
    let budget = AllocationBudget::new(1 << 20);
    let target = fixture(&budget);
    let before = budget.reserved_bytes();
    budget.with_deferred_refund_notifications(|scope| {
        let blocker = target.blocks.try_acquire_writer().unwrap();
        let expected = target.blocks_released.observe();
        let mut slot = target.try_block_acquisition_admitted(scope).unwrap();
        let _healthy = StartFault::set(0);
        match slot.try_initialize(BlockMode::Replace) {
            Err(AdmittedStorageError::Busy {
                role: StorageRole::Current,
                release,
            }) => assert_eq!(release, expected),
            other => panic!("lost original current wait: {other:?}"),
        }
        assert_eq!(START_CALLS.with(Cell::get), 0);
        assert!(target.revert.try_acquire_writer().is_none());
        slot.release();
        assert!(target.revert.try_acquire_writer().is_some());
        drop((slot, blocker));
    });
    assert_eq!(budget.reserved_bytes(), before);
    drop(target);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn scoped_original_block_capture_releases_scope_and_keeps_replacement_custody() {
    let budget = AllocationBudget::new(1 << 20);
    let target = fixture(&budget);
    let retained = target.view();
    let journal = budget.with_deferred_refund_notifications(|scope| {
        let mut slot = target.try_block_acquisition_admitted(scope).unwrap();
        slot.try_initialize(BlockMode::Replace).unwrap();
        let mut block = slot.into_block();
        assert_eq!(block.get(&7), Some(&70));
        assert_eq!(block.mode(), BlockMode::Replace);
        block.try_insert_admitted(7, 73).unwrap();
        let mut capture = block.capture_slot();
        capture.try_capture(|_| Ok::<_, ()>(())).unwrap();
        let (journal, cleanup) = capture.into_detached();
        assert!(target.revert.try_acquire_writer().is_some());
        assert!(target.blocks.try_acquire_writer().is_some());
        drop(cleanup);
        journal
    });
    assert_eq!(target.view().get(&7), Some(&71));
    budget.with_deferred_refund_notifications(|scope| {
        let prepared = match journal.try_prepare_admitted(scope, &target) {
            Ok(prepared) => prepared,
            Err(_) => panic!("unchanged original target"),
        };
        prepared.publish().into_admission();
    });
    assert_eq!(target.view().get(&7), Some(&73));
    assert_eq!(target.revert.read().get(&7), Some(&Some(70)));
    assert_eq!(retained.get(&7), Some(&71));
    assert!(budget.reserved_bytes() > 0);
    drop(retained);
    drop(target);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn ordinary_acquisition_retains_zero_sized_scope_and_original_protocol() {
    assert_eq!(
        std::mem::size_of::<<Untracked as StorageMode<u64, u64>>::AcquisitionCustody>(),
        0
    );
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<<Untracked as StorageMode<u64, u64>>::AcquisitionCustody>();
    fn narrow<'short, 'long: 'short>(block: Block<'long, u64, u64>) -> Block<'short, u64, u64> {
        block
    }
    let target = Storage::from_iter([(7u64, 70u64)]);
    let mut slot = target.block_acquisition();
    slot.initialize(BlockMode::Ordinary);
    let mut block = narrow(slot.into_block());
    block.insert(7, 71);
    block.commit();
    assert_eq!(target.view().get(&7), Some(&71));
    assert_eq!(target.revert.read().get(&7), Some(&Some(70)));
}
