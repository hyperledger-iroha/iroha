//! Fresh current/undo writer construction owns both locks before callbacks.

use super::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
};

thread_local! {
    static START_CALLS: Cell<usize> = const { Cell::new(0) };
    // 0: healthy; 1: second provider changes demand; 2: second provider panics.
    static START_FAULT: Cell<u8> = const { Cell::new(0) };
}

struct StartFault;
impl StartFault {
    fn set(mode: u8) -> Self {
        START_CALLS.with(|calls| calls.set(0));
        START_FAULT.with(|fault| fault.set(mode));
        Self
    }
}
impl Drop for StartFault {
    fn drop(&mut self) {
        START_FAULT.with(|fault| fault.set(0));
    }
}

struct StartPolicy(AllocationReservation);
impl NodeFunding for StartPolicy {
    type Charge = AllocationCharge;
    fn take_node_charge(&mut self, layout: Layout) -> AllocationCharge {
        self.0
            .try_split(layout)
            .expect("original complete start demand")
    }
}
impl<V: Copy> NodeCloning<u64, V> for StartPolicy {
    fn clone_key(&mut self, key: &u64) -> u64 {
        *key
    }
    fn clone_value(&mut self, value: &V) -> V {
        *value
    }
}
impl<V: Copy> ClonePlanning<u64, V> for StartPolicy {
    fn plan_key(_: &u64, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &V, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}
impl AdmittedStoragePolicy for StartPolicy {
    fn from_admission(mut reservation: AllocationReservation) -> Self {
        let call = START_CALLS.with(|calls| {
            calls.set(calls.get() + 1);
            calls.get()
        });
        if call == 2 {
            match START_FAULT.with(Cell::get) {
                0 => {}
                1 => drop(
                    reservation
                        .try_partition_bytes(1)
                        .expect("nonempty cursor demand"),
                ),
                2 => panic!("second original writer provider failed"),
                _ => unreachable!(),
            }
        }
        Self(reservation)
    }
    fn admission(&self) -> &AllocationReservation {
        &self.0
    }
}

type StartStorage = Storage<u64, u64, Prepaid<StartPolicy>>;

struct Probe<M: StorageMode<u64, u64>> {
    target: Arc<Storage<u64, u64, M>>,
    calls: AtomicUsize,
    // Bit 0: undo unavailable; bit 1: current unavailable.
    unavailable: AtomicUsize,
    last_unavailable: AtomicUsize,
}
impl<M: StorageMode<u64, u64>> Probe<M> {
    fn new(target: &Arc<Storage<u64, u64, M>>) -> Arc<Self> {
        Arc::new(Self {
            target: Arc::clone(target),
            calls: AtomicUsize::new(0),
            unavailable: AtomicUsize::new(0),
            last_unavailable: AtomicUsize::new(0),
        })
    }
}
impl<M> Wake for Probe<M>
where
    M: StorageMode<u64, u64> + Send + Sync + 'static,
    M::Charge: Send + Sync,
{
    fn wake(self: Arc<Self>) {
        // No assertion or blocking acquisition in Wake: the constructor may
        // already be unwinding. Poisoned-but-unlocked is still Some here.
        let undo = self.target.revert.try_acquire_writer();
        let current = self.target.blocks.try_acquire_writer();
        let unavailable = usize::from(undo.is_none()) | (usize::from(current.is_none()) << 1);
        drop((undo, current));
        self.unavailable.fetch_or(unavailable, SeqCst);
        self.last_unavailable.store(unavailable, SeqCst);
        self.calls.fetch_add(1, SeqCst);
    }
}

fn register<M>(
    target: &Arc<Storage<u64, u64, M>>,
    probe: &Arc<Probe<M>>,
) -> (
    crate::ReleaseWait,
    crate::ReleaseWait,
    concread::release::ReleaseFuture,
    concread::release::ReleaseFuture,
)
where
    M: StorageMode<u64, u64> + Send + Sync + 'static,
    M::Charge: Send + Sync,
{
    let undo = target.revert_released.observe();
    let current = target.blocks_released.observe();
    let mut undo_future = undo.clone().wait_for_release();
    let mut current_future = current.clone().wait_for_release();
    let waker = Waker::from(Arc::clone(probe));
    let mut context = Context::from_waker(&waker);
    assert!(Pin::new(&mut undo_future).poll(&mut context).is_pending());
    assert!(
        Pin::new(&mut current_future)
            .poll(&mut context)
            .is_pending()
    );
    (undo, current, undo_future, current_future)
}

fn ready<M>(future: &mut concread::release::ReleaseFuture, probe: &Arc<Probe<M>>) -> bool
where
    M: StorageMode<u64, u64> + Send + Sync + 'static,
    M::Charge: Send + Sync,
{
    // Polling a pending future updates its registered waker. Keep the real
    // probe installed rather than replacing it with a no-op during checks.
    let waker = Waker::from(Arc::clone(probe));
    Pin::new(future)
        .poll(&mut Context::from_waker(&waker))
        .is_ready()
}

fn fixture(budget: &AllocationBudget) -> Arc<StartStorage> {
    let _healthy = StartFault::set(0);
    let target = Arc::new(StartStorage::try_new_admitted(budget.clone()).unwrap());
    for value in [70, 71] {
        target
            .try_with_admitted_block(|block| {
                block.try_insert_admitted(7, value).unwrap();
                Ok::<_, ()>(())
            })
            .unwrap();
    }
    target
}

fn assert_original(target: &StartStorage, predecessor: &CapturedPublication) {
    assert_eq!(target.view().get(&7), Some(&71));
    assert_eq!(target.revert.read().get(&7), Some(&Some(70)));
    let (verdict, cleanup) = predecessor.try_check_current::<()>(&target.publication);
    drop(cleanup);
    assert_eq!(verdict, Ok(()));
}

fn refused_start(mode: u8) {
    for replacement in [false, true] {
        let budget = AllocationBudget::new(1 << 20);
        let target = fixture(&budget);
        let predecessor = target.publication.capture();
        let before = budget.reserved_bytes();
        let probe = Probe::new(&target);
        let (undo, current, mut undo_future, mut current_future) = register(&target, &probe);
        let callback_ran = Cell::new(false);
        let fault = StartFault::set(mode);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let callback = |_: &mut Block<'_, u64, u64, Prepaid<StartPolicy>>| {
                callback_ran.set(true);
                Ok::<_, ()>(())
            };
            if replacement {
                target.try_with_admitted_replacement(callback)
            } else {
                target.try_with_admitted_block(callback)
            }
        }));
        assert_eq!(START_CALLS.with(Cell::get), 2);
        drop(fault);
        assert!(!callback_ran.get());
        match (mode, result) {
            (
                1,
                Ok(Err(AdmittedBlockError::Admission(AdmittedStorageError::PolicyDemand {
                    expected_bytes,
                    remaining_bytes,
                }))),
            ) => {
                assert_eq!(expected_bytes, remaining_bytes + 1);
            }
            (2, Err(_)) => {}
            (_, result) => panic!("wrong second-provider outcome: {result:?}"),
        }
        assert_eq!(probe.calls.load(SeqCst), 2);
        assert_eq!(
            probe.unavailable.load(SeqCst),
            0,
            "a release callback saw a held sibling"
        );
        assert!(ready(&mut undo_future, &probe) && ready(&mut current_future, &probe));
        assert_eq!(
            (undo.is_poisoned(), current.is_poisoned()),
            (mode == 2, mode == 2)
        );
        assert_eq!(
            (target.revert.is_poisoned(), target.blocks.is_poisoned()),
            (mode == 2, mode == 2)
        );
        assert_eq!(budget.reserved_bytes(), before);
        assert_original(&target, &predecessor);
        if mode == 1 {
            assert!(matches!(
                target.try_with_admitted_block(|_| Err::<(), _>(17)),
                Err(AdmittedBlockError::Callback(17))
            ));
            assert_original(&target, &predecessor);
        } else {
            assert!(matches!(
                target.try_with_admitted_block(|_| Ok::<_, ()>(())),
                Err(AdmittedBlockError::Admission(
                    AdmittedStorageError::Poisoned { .. }
                ))
            ));
        }
        drop((
            undo_future,
            current_future,
            undo,
            current,
            predecessor,
            probe,
            target,
        ));
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn admitted_second_policy_refusal_releases_both_before_callbacks() {
    refused_start(1);
}

#[test]
fn admitted_second_policy_panic_releases_both_before_callbacks() {
    refused_start(2);
}

#[test]
fn ordinary_current_poison_releases_both_before_callbacks() {
    for replacement in [false, true] {
        let target = Arc::new(Storage::<u64, u64>::from_iter([(7, 71)]));
        let predecessor = target.publication.capture();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _current = target.blocks.write();
                panic!("poison original current before pair acquisition");
            }))
            .is_err()
        );
        let probe = Probe::new(&target);
        let (undo, current, mut undo_future, mut current_future) = register(&target, &probe);
        let result = catch_unwind(AssertUnwindSafe(|| {
            if replacement {
                drop(target.block_and_revert());
            } else {
                drop(target.block());
            }
        }));
        assert!(result.is_err());
        assert_eq!(probe.calls.load(SeqCst), 2);
        assert_eq!(
            probe.unavailable.load(SeqCst),
            0,
            "current constructor wake preceded undo unlock"
        );
        assert!(ready(&mut undo_future, &probe) && ready(&mut current_future, &probe));
        assert!(undo.is_poisoned() && current.is_poisoned());
        assert!(target.revert.is_poisoned() && target.blocks.is_poisoned());
        assert_eq!(target.view().get(&7), Some(&71));
        assert!(target.revert.read().is_empty());
        let (verdict, cleanup) = predecessor.try_check_current::<()>(&target.publication);
        drop(cleanup);
        assert_eq!(verdict, Ok(()));
    }
}

#[test]
fn current_busy_releases_only_acquired_undo() {
    for replacement in [false, true] {
        let budget = AllocationBudget::new(1 << 20);
        let target = fixture(&budget);
        let before = budget.reserved_bytes();
        let predecessor = target.publication.capture();
        let held = target
            .blocks_released
            .poisoning_guard(target.blocks.try_acquire_writer().unwrap());
        let probe = Probe::new(&target);
        let (undo, current, mut undo_future, mut current_future) = register(&target, &probe);
        let _healthy = StartFault::set(0);
        let callback = |_: &mut Block<'_, u64, u64, Prepaid<StartPolicy>>| -> Result<(), ()> {
            panic!("busy current cannot execute")
        };
        let result = if replacement {
            target.try_with_admitted_replacement(callback)
        } else {
            target.try_with_admitted_block(callback)
        };
        let Err(AdmittedBlockError::Admission(AdmittedStorageError::Busy { role, release })) =
            result
        else {
            panic!("current must report its original Busy");
        };
        assert_eq!(role, StorageRole::Current);
        assert_eq!(release, current);
        assert_eq!(
            START_CALLS.with(Cell::get),
            0,
            "both raw acquisitions precede either policy"
        );
        assert_eq!(probe.calls.load(SeqCst), 1);
        assert_eq!(probe.unavailable.load(SeqCst), 2);
        assert!(ready(&mut undo_future, &probe));
        assert!(!ready(&mut current_future, &probe));
        assert!(!undo.is_poisoned() && !current.is_poisoned());
        assert_eq!(budget.reserved_bytes(), before);
        drop(held);
        assert_eq!(probe.calls.load(SeqCst), 2);
        assert_eq!(probe.last_unavailable.load(SeqCst), 0);
        assert!(ready(&mut current_future, &probe));
        assert_original(&target, &predecessor);
        assert!(matches!(
            target.try_with_admitted_block(|_| Err::<(), _>(17)),
            Err(AdmittedBlockError::Callback(17))
        ));
        drop((
            release,
            undo_future,
            current_future,
            undo,
            current,
            predecessor,
            probe,
            target,
        ));
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn successful_pair_construction_emits_no_early_release() {
    for replacement in [false, true] {
        let budget = AllocationBudget::new(1 << 20);
        let target = fixture(&budget);
        let before = budget.reserved_bytes();
        let predecessor = target.publication.capture();
        let probe = Probe::new(&target);
        let (undo, current, mut undo_future, mut current_future) = register(&target, &probe);
        let callback = |block: &mut Block<'_, u64, u64, Prepaid<StartPolicy>>| {
            assert!(target.revert.try_acquire_writer().is_none());
            assert!(target.blocks.try_acquire_writer().is_none());
            assert_eq!(probe.calls.load(SeqCst), 0);
            assert!(!ready(&mut undo_future, &probe) && !ready(&mut current_future, &probe));
            assert_eq!(
                block.mode(),
                if replacement {
                    BlockMode::Replace
                } else {
                    BlockMode::Ordinary
                }
            );
            assert_eq!(block.get(&7), Some(if replacement { &70 } else { &71 }));
            assert!(block.revert_map().is_empty());
            Err::<(), _>(17)
        };
        let result = if replacement {
            target.try_with_admitted_replacement(callback)
        } else {
            target.try_with_admitted_block(callback)
        };
        assert!(matches!(result, Err(AdmittedBlockError::Callback(17))));
        assert_eq!(probe.calls.load(SeqCst), 2);
        assert_eq!(probe.unavailable.load(SeqCst), 0);
        assert!(ready(&mut undo_future, &probe) && ready(&mut current_future, &probe));
        assert!(!undo.is_poisoned() && !current.is_poisoned());
        assert_eq!(budget.reserved_bytes(), before);
        assert_original(&target, &predecessor);
        drop((
            undo_future,
            current_future,
            undo,
            current,
            predecessor,
            probe,
            target,
        ));
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
#[test]
fn admitted_refusal_wake_panic_preserves_healthy_pair_and_surviving_waiters() {
    struct PanicWake {
        probe: Arc<Probe<Prepaid<StartPolicy>>>,
    }
    impl Wake for PanicWake {
        fn wake(self: Arc<Self>) {
            Arc::clone(&self.probe).wake();
            panic!("injected wake after original physical release");
        }
    }
    let budget = AllocationBudget::new(1 << 20);
    let target = fixture(&budget);
    let before = budget.reserved_bytes();
    let predecessor = target.publication.capture();
    let probe = Probe::new(&target);
    let panic_waker = Waker::from(Arc::new(PanicWake {
        probe: Arc::clone(&probe),
    }));
    let mut first_undo = target.revert_released.observe().wait_for_release();
    assert!(
        Pin::new(&mut first_undo)
            .poll(&mut Context::from_waker(&panic_waker))
            .is_pending()
    );
    let (undo, current, mut undo_future, mut current_future) = register(&target, &probe);
    let fault = StartFault::set(1);
    let callback_ran = Cell::new(false);
    let result = catch_unwind(AssertUnwindSafe(|| {
        target.try_with_admitted_block(|_| {
            callback_ran.set(true);
            Ok::<_, ()>(())
        })
    }));
    drop(fault);
    assert!(result.is_err(), "the original waker panic must propagate");
    assert!(!callback_ran.get());
    assert_eq!(START_CALLS.with(Cell::get), 2);
    // The remaining undo cohort and the current cohort both survive the
    // first wake panic. Every probe sees both physical writers unlocked.
    assert_eq!(probe.calls.load(SeqCst), 3);
    assert_eq!(probe.unavailable.load(SeqCst), 0);
    assert!(ready(&mut first_undo, &probe));
    assert!(ready(&mut undo_future, &probe) && ready(&mut current_future, &probe));
    assert!(!undo.is_poisoned() && !current.is_poisoned());
    assert!(!target.revert.is_poisoned() && !target.blocks.is_poisoned());
    assert_eq!(budget.reserved_bytes(), before);
    assert_original(&target, &predecessor);
    assert!(matches!(
        target.try_with_admitted_block(|_| Err::<(), _>(17)),
        Err(AdmittedBlockError::Callback(17))
    ));
    assert_original(&target, &predecessor);
    drop((
        panic_waker,
        first_undo,
        undo_future,
        current_future,
        undo,
        current,
        predecessor,
        probe,
        target,
    ));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn admitted_undo_poison_precedes_busy_current_without_policy() {
    for replacement in [false, true] {
        let budget = AllocationBudget::new(1 << 20);
        let target = fixture(&budget);
        let predecessor = target.publication.capture();
        let before = budget.reserved_bytes();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _undo = target.revert.try_acquire_writer().unwrap();
                panic!("poison undo without constructing a cursor");
            }))
            .is_err()
        );
        let held = target
            .blocks_released
            .poisoning_guard(target.blocks.try_acquire_writer().unwrap());
        let probe = Probe::new(&target);
        let (undo, current, mut undo_future, mut current_future) = register(&target, &probe);
        let fault = StartFault::set(0);
        let callback = |_: &mut Block<'_, u64, u64, Prepaid<StartPolicy>>| -> Result<(), ()> {
            panic!("poisoned undo cannot execute");
        };
        let result = if replacement {
            target.try_with_admitted_replacement(callback)
        } else {
            target.try_with_admitted_block(callback)
        };
        assert!(matches!(
            result,
            Err(AdmittedBlockError::Admission(
                AdmittedStorageError::Poisoned {
                    role: StorageRole::Undo
                }
            ))
        ));
        assert_eq!(START_CALLS.with(Cell::get), 0);
        drop(fault);
        assert_eq!(probe.calls.load(SeqCst), 1, "only undo was acquired");
        // Current is held independently by this test; the released undo must
        // already be available when its original callback executes.
        assert_eq!(probe.unavailable.load(SeqCst), 2);
        assert!(ready(&mut undo_future, &probe));
        assert!(!ready(&mut current_future, &probe));
        assert!(undo.is_poisoned());
        assert!(!current.is_poisoned());
        assert_eq!(budget.reserved_bytes(), before);
        drop(held);
        assert_eq!(probe.calls.load(SeqCst), 2);
        assert_eq!(probe.last_unavailable.load(SeqCst), 0);
        assert!(ready(&mut current_future, &probe));
        assert!(!target.blocks.is_poisoned());
        assert_original(&target, &predecessor);
        drop((
            undo_future,
            current_future,
            undo,
            current,
            predecessor,
            probe,
            target,
        ));
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn ordinary_undo_poison_does_not_wait_for_current() {
    use std::{sync::mpsc, time::Duration};

    for replacement in [false, true] {
        let target = Arc::new(Storage::<u64, u64>::from_iter([(7, 71)]));
        let predecessor = target.publication.capture();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _undo = target.revert.try_acquire_writer().unwrap();
                panic!("poison original undo without touching current");
            }))
            .is_err()
        );
        let held = target
            .blocks_released
            .poisoning_guard(target.blocks.try_acquire_writer().unwrap());
        let probe = Probe::new(&target);
        let (undo, current, mut undo_future, mut current_future) = register(&target, &probe);
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (finished_tx, finished_rx) = mpsc::sync_channel(1);
        let worker_target = Arc::clone(&target);
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let rejected = catch_unwind(AssertUnwindSafe(|| {
                if replacement {
                    drop(worker_target.block_and_revert());
                } else {
                    drop(worker_target.block());
                }
            }))
            .is_err();
            finished_tx.send(rejected).unwrap();
        });
        let started = started_rx.recv_timeout(Duration::from_secs(5));
        let finished_before_release = finished_rx.recv_timeout(Duration::from_secs(2));
        let calls_before_release = probe.calls.load(SeqCst);
        let undo_ready_before_release = ready(&mut undo_future, &probe);
        let current_ready_before_release = ready(&mut current_future, &probe);
        // Always unblock and join the worker before any assertion. The broken
        // ordering therefore fails boundedly instead of stranding a thread.
        drop(held);
        let joined = worker.join();
        assert!(started.is_ok());
        assert!(joined.is_ok());
        assert!(
            matches!(finished_before_release, Ok(true)),
            "known undo poison waited for unrelated current: {finished_before_release:?}"
        );
        assert_eq!(calls_before_release, 1);
        assert!(undo_ready_before_release && !current_ready_before_release);
        assert_eq!(probe.calls.load(SeqCst), 2);
        assert_eq!(probe.unavailable.load(SeqCst), 2);
        assert_eq!(probe.last_unavailable.load(SeqCst), 0);
        assert!(undo.is_poisoned() && !current.is_poisoned());
        assert!(target.revert.is_poisoned() && !target.blocks.is_poisoned());
        assert_eq!(target.view().get(&7), Some(&71));
        assert!(target.revert.read().is_empty());
        let (verdict, cleanup) = predecessor.try_check_current::<()>(&target.publication);
        drop(cleanup);
        assert_eq!(verdict, Ok(()));
    }
}
