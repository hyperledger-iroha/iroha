//! Exact retained pairs keep preparation cleanup behind all physical siblings.

use crate::{
    BlockMode, PublicationPreparationError as Refusal, ReleaseWait,
    cell::{self, Cell},
    storage::{self, Storage},
};
use concread::release::ReleaseFuture;
use std::{
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering::SeqCst},
    },
    task::{Context, Wake, Waker},
};

struct Targets {
    cell: Cell<u64>,
    map: Storage<u64, u64>,
}
fn seeded() -> Arc<Targets> {
    let target = Arc::new(Targets {
        cell: Cell::new(10),
        map: Storage::from_iter([(1, 10)]),
    });
    let mut c = target.cell.block();
    *c.get_mut() = 20;
    c.commit();
    let mut m = target.map.block();
    m.insert(1, 20);
    m.commit();
    target
}
fn originals(
    target: &Targets,
    mode: BlockMode,
) -> (cell::Detached<u64, ()>, storage::Detached<u64, u64, ()>) {
    let mut c = match mode {
        BlockMode::Ordinary => target.cell.block(),
        BlockMode::Replace => target.cell.block_and_revert(),
    };
    let mut m = match mode {
        BlockMode::Ordinary => target.map.block(),
        BlockMode::Replace => target.map.block_and_revert(),
    };
    *c.get_mut() = 33;
    m.insert(1, 33);
    (
        c.try_detach(|_| Ok::<_, ()>(())).unwrap(),
        m.try_detach(|_| Ok::<_, ()>(())).unwrap(),
    )
}
struct Pair<'a> {
    cell: Option<cell::DetachedPublicationSlot<'a, u64, (), ()>>,
    map: Option<storage::DetachedPublicationSlot<'a, u64, u64, (), ()>>,
}
impl Drop for Pair<'_> {
    fn drop(&mut self) {
        if let Some(c) = &mut self.cell {
            c.release_writers();
        }
        if let Some(m) = &mut self.map {
            m.release_writers();
        }
    }
}
fn pair<'a>(
    target: &'a Targets,
    c: cell::Detached<u64, ()>,
    m: storage::Detached<u64, u64, ()>,
) -> Pair<'a> {
    Pair {
        cell: Some(c.publication_slot(&target.cell)),
        map: Some(m.publication_slot(&target.map)),
    }
}
struct Probe {
    target: Arc<Targets>,
    calls: AtomicUsize,
    held: AtomicUsize,
    poisoned: AtomicUsize,
}
impl Wake for Probe {
    fn wake(self: Arc<Self>) {
        let cu = self.target.cell.revert.try_acquire_writer();
        let cc = self.target.cell.blocks.try_acquire_writer();
        let mu = self.target.map.revert.try_acquire_writer();
        let mc = self.target.map.blocks.try_acquire_writer();
        let ci = self.target.cell.publication.version.try_lock();
        let mi = self.target.map.publication.version.try_lock();
        self.held.fetch_add(
            usize::from(cu.is_none())
                + usize::from(cc.is_none())
                + usize::from(mu.is_none())
                + usize::from(mc.is_none())
                + usize::from(matches!(ci, Err(std::sync::TryLockError::WouldBlock)))
                + usize::from(matches!(mi, Err(std::sync::TryLockError::WouldBlock))),
            SeqCst,
        );
        self.poisoned.fetch_add(
            usize::from(cu.as_ref().is_some_and(|x| x.is_poisoned()))
                + usize::from(cc.as_ref().is_some_and(|x| x.is_poisoned()))
                + usize::from(mu.as_ref().is_some_and(|x| x.is_poisoned()))
                + usize::from(mc.as_ref().is_some_and(|x| x.is_poisoned())),
            SeqCst,
        );
        self.calls.fetch_add(1, SeqCst);
        // Only genuine nonblocking physical probes; assertions stay outside Wake.
    }
}
fn arm(target: &Arc<Targets>, wait: ReleaseWait) -> (Arc<Probe>, ReleaseFuture, Waker) {
    let probe = Arc::new(Probe {
        target: Arc::clone(target),
        calls: AtomicUsize::new(0),
        held: AtomicUsize::new(0),
        poisoned: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&probe));
    let mut future = wait.wait_for_release();
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    (probe, future, waker)
}
fn complete(probe: &Probe, mut future: ReleaseFuture, waker: &Waker) {
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(waker))
            .is_ready()
    );
    assert_eq!(probe.calls.load(SeqCst), 1);
    assert_eq!(
        probe.held.load(SeqCst),
        0,
        "notification observed an original physical sibling"
    );
}

#[test]
fn detached_slots_preserve_exact_originals_retry_and_publication_in_both_modes() {
    for mode in [BlockMode::Ordinary, BlockMode::Replace] {
        let target = seeded();
        let old_cell = target.cell.view();
        let old_map = target.map.view();
        let (c, m) = originals(&target, mode);
        let cp = std::ptr::from_ref(c.touched_value().unwrap().after);
        let mp = std::ptr::from_ref(m.touched_entries().next().unwrap().after.unwrap());
        let mut slots = pair(&target, c, m);
        let (probe, future, waker) = arm(&target, target.cell.revert_released.observe());
        slots
            .cell
            .as_mut()
            .unwrap()
            .try_prepare(|_, _| Ok::<_, ()>(()))
            .unwrap();
        slots
            .map
            .as_mut()
            .unwrap()
            .try_prepare(|_, _| Ok::<_, ()>(()))
            .unwrap();
        let c = slots.cell.as_mut().unwrap().recover_original();
        let m = slots.map.as_mut().unwrap().recover_original();
        assert_eq!(probe.calls.load(SeqCst), 0);
        assert_eq!(std::ptr::from_ref(c.touched_value().unwrap().after), cp);
        assert_eq!(
            std::ptr::from_ref(m.touched_entries().next().unwrap().after.unwrap()),
            mp
        );
        assert_eq!(c.mode(), mode);
        assert_eq!(m.mode(), mode);
        drop(slots);
        complete(&probe, future, &waker);
        let mut slots = pair(&target, c, m);
        let (probe, future, waker) = arm(&target, target.cell.revert_released.observe());
        slots
            .cell
            .as_mut()
            .unwrap()
            .try_prepare(|_, _| Ok::<_, ()>(()))
            .unwrap();
        slots
            .map
            .as_mut()
            .unwrap()
            .try_prepare(|_, _| Ok::<_, ()>(()))
            .unwrap();
        let c = slots.cell.take().unwrap().into_prepared().publish();
        assert_eq!(probe.calls.load(SeqCst), 0);
        let m = slots.map.take().unwrap().into_prepared().publish();
        assert_eq!(probe.calls.load(SeqCst), 0);
        assert_eq!(*target.cell.view(), 33);
        assert_eq!(target.map.view().get(&1), Some(&33));
        assert_eq!(*old_cell, 20);
        assert_eq!(old_map.get(&1), Some(&20));
        drop((slots, c, m));
        complete(&probe, future, &waker);
        assert_eq!(probe.poisoned.load(SeqCst), 0);
    }
}

#[test]
fn detached_slots_retain_late_admission_refusal_and_caught_panic_cleanup() {
    for panic in [false, true] {
        let target = seeded();
        let (c, m) = originals(&target, BlockMode::Ordinary);
        let mut slots = pair(&target, c, m);
        let (probe, future, waker) = arm(&target, target.map.publication.released.observe());
        slots
            .cell
            .as_mut()
            .unwrap()
            .try_prepare(|_, _| Ok::<_, ()>(()))
            .unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            slots.map.as_mut().unwrap().try_prepare(|_, _| {
                if panic {
                    panic!("installation admission unwind");
                }
                Err::<(), _>("capacity")
            })
        }));
        if panic {
            assert!(result.is_err());
            assert!(
                catch_unwind(AssertUnwindSafe(|| slots
                    .map
                    .as_mut()
                    .unwrap()
                    .recover_original()))
                .is_err()
            );
        } else {
            assert_eq!(result.unwrap(), Err(Refusal::Admission("capacity")));
            let original = slots.map.as_mut().unwrap().recover_original();
            assert_eq!(original.touched_entries().next().unwrap().after, Some(&33));
            drop(original);
        }
        assert_eq!(probe.calls.load(SeqCst), 0);
        assert!(target.cell.blocks.try_acquire_writer().is_none());
        drop(slots);
        complete(&probe, future, &waker);
        assert_eq!(
            probe.poisoned.load(SeqCst),
            0,
            "caught admission panic did not unwind raw owners"
        );
        assert_eq!(*target.cell.view(), 20);
        assert_eq!(target.map.view().get(&1), Some(&20));
    }
    // Exercise the independent EBR slot with a later admission panic too.
    let target = seeded();
    let (c, m) = originals(&target, BlockMode::Replace);
    let mut slots = pair(&target, c, m);
    let (probe, future, waker) = arm(&target, target.cell.publication.released.observe());
    slots
        .map
        .as_mut()
        .unwrap()
        .try_prepare(|_, _| Ok::<_, ()>(()))
        .unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _: Result<(), Refusal<()>> = slots
                .cell
                .as_mut()
                .unwrap()
                .try_prepare(|_, _| panic!("later EBR admission"));
        }))
        .is_err()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| slots
            .cell
            .as_mut()
            .unwrap()
            .recover_original()))
        .is_err()
    );
    assert_eq!(probe.calls.load(SeqCst), 0);
    assert!(target.map.blocks.try_acquire_writer().is_none());
    drop(slots);
    complete(&probe, future, &waker);
    assert_eq!(probe.poisoned.load(SeqCst), 0);
}

#[test]
fn detached_slots_current_busy_retains_acquired_undo_until_original_retry() {
    let target = seeded();
    let (c, m) = originals(&target, BlockMode::Ordinary);
    let original = std::ptr::from_ref(m.touched_entries().next().unwrap().after.unwrap());
    let mut slots = pair(&target, c, m);
    let current = target.map.blocks.try_acquire_writer().unwrap();
    let (probe, future, waker) = arm(&target, target.map.revert_released.observe());
    slots
        .cell
        .as_mut()
        .unwrap()
        .try_prepare(|_, _| Ok::<_, ()>(()))
        .unwrap();
    assert!(matches!(
        slots
            .map
            .as_mut()
            .unwrap()
            .try_prepare(|_, _| Ok::<_, ()>(())),
        Err(Refusal::Busy(_))
    ));
    assert!(target.map.revert.try_acquire_writer().is_none());
    assert_eq!(probe.calls.load(SeqCst), 0);
    let c = slots.cell.as_mut().unwrap().recover_original();
    let m = slots.map.as_mut().unwrap().recover_original();
    assert_eq!(
        std::ptr::from_ref(m.touched_entries().next().unwrap().after.unwrap()),
        original
    );
    assert_eq!(probe.calls.load(SeqCst), 0);
    drop(current);
    drop(slots);
    complete(&probe, future, &waker);
    let mut slots = pair(&target, c, m);
    slots
        .cell
        .as_mut()
        .unwrap()
        .try_prepare(|_, _| Ok::<_, ()>(()))
        .unwrap();
    slots
        .map
        .as_mut()
        .unwrap()
        .try_prepare(|_, _| Ok::<_, ()>(()))
        .unwrap();
    drop(slots);
}

#[test]
fn detached_slots_late_identity_refusal_retains_native_reader_and_writer_cleanup() {
    let target = seeded();
    let (c, m) = originals(&target, BlockMode::Replace);
    let mut held = None;
    let mut slots = pair(&target, c, m);
    let (probe, future, waker) = arm(&target, target.map.blocks.observe_reader_release());
    slots
        .cell
        .as_mut()
        .unwrap()
        .try_prepare(|_, _| Ok::<_, ()>(()))
        .unwrap();
    assert!(matches!(
        slots.map.as_mut().unwrap().try_prepare(|_, _| {
            held = Some(target.map.publication.version.lock().unwrap());
            Ok::<_, ()>(())
        }),
        Err(Refusal::Busy(_))
    ));
    assert!(matches!(
        target.map.blocks.try_read(),
        Err(concread::bptree::OwnedWriteError::Busy)
    ));
    let c = slots.cell.as_mut().unwrap().recover_original();
    let m = slots.map.as_mut().unwrap().recover_original();
    assert_eq!(probe.calls.load(SeqCst), 0);
    drop(held);
    drop(slots);
    complete(&probe, future, &waker);
    let mut slots = pair(&target, c, m);
    slots
        .cell
        .as_mut()
        .unwrap()
        .try_prepare(|_, _| Ok::<_, ()>(()))
        .unwrap();
    slots
        .map
        .as_mut()
        .unwrap()
        .try_prepare(|_, _| Ok::<_, ()>(()))
        .unwrap();
    drop(slots);
}

#[test]
fn detached_slots_known_native_poison_is_retained_without_early_notification() {
    let target = seeded();
    let (c, m) = originals(&target, BlockMode::Ordinary);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _held = target.map.blocks.try_acquire_writer().unwrap();
            panic!("original writer poison");
        }))
        .is_err()
    );
    let mut slots = pair(&target, c, m);
    let observed = target.map.blocks_released.observe();
    let (probe, future, waker) = arm(&target, observed.clone());
    slots
        .cell
        .as_mut()
        .unwrap()
        .try_prepare(|_, _| Ok::<_, ()>(()))
        .unwrap();
    assert_eq!(
        slots
            .map
            .as_mut()
            .unwrap()
            .try_prepare(|_, _| Ok::<_, ()>(())),
        Err(Refusal::Poisoned)
    );
    assert!(target.map.revert.try_acquire_writer().is_none());
    assert_eq!(probe.calls.load(SeqCst), 0);
    drop(slots);
    complete(&probe, future, &waker);
    assert!(observed.is_poisoned());
    assert_eq!(probe.poisoned.load(SeqCst), 1);
    // Poison on the EBR raw phase is equally distinct from busy or foreign.
    let target = seeded();
    let (c, m) = originals(&target, BlockMode::Ordinary);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _held = target.cell.blocks.try_acquire_writer().unwrap();
            panic!("original EBR writer poison");
        }))
        .is_err()
    );
    let mut slots = pair(&target, c, m);
    let observed = target.cell.blocks_released.observe();
    let (probe, future, waker) = arm(&target, observed.clone());
    slots
        .map
        .as_mut()
        .unwrap()
        .try_prepare(|_, _| Ok::<_, ()>(()))
        .unwrap();
    assert_eq!(
        slots
            .cell
            .as_mut()
            .unwrap()
            .try_prepare(|_, _| Ok::<_, ()>(())),
        Err(Refusal::Poisoned)
    );
    assert!(target.cell.revert.try_acquire_writer().is_none());
    assert_eq!(probe.calls.load(SeqCst), 0);
    drop(slots);
    complete(&probe, future, &waker);
    assert!(observed.is_poisoned());
    assert_eq!(probe.poisoned.load(SeqCst), 1);
}

#[test]
fn detached_slots_outer_unwind_releases_siblings_before_original_poison_wakes() {
    let target = seeded();
    let (c, m) = originals(&target, BlockMode::Ordinary);
    let observed = target.cell.revert_released.observe();
    let (probe, future, waker) = arm(&target, observed.clone());
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut slots = pair(&target, c, m);
            slots
                .cell
                .as_mut()
                .unwrap()
                .try_prepare(|_, _| Ok::<_, ()>(()))
                .unwrap();
            let _: Result<(), Refusal<()>> = slots
                .map
                .as_mut()
                .unwrap()
                .try_prepare(|_, _| panic!("late admission unwinds aggregate"));
        }))
        .is_err()
    );
    complete(&probe, future, &waker);
    assert!(observed.is_poisoned());
    assert_eq!(
        probe.poisoned.load(SeqCst),
        2,
        "only acquired Cell writers unwound"
    );
}

struct Scalar(crate::allocation::AllocationReservation);
impl concread::bptree::NodeFunding for Scalar {
    type Charge = crate::allocation::AllocationCharge;
    fn take_node_charge(&mut self, layout: std::alloc::Layout) -> Self::Charge {
        self.0.try_split(layout).unwrap()
    }
}
impl<V: Copy> concread::bptree::NodeCloning<u64, V> for Scalar {
    fn clone_key(&mut self, key: &u64) -> u64 {
        *key
    }
    fn clone_value(&mut self, value: &V) -> V {
        *value
    }
}
impl<V: Copy> concread::bptree::ClonePlanning<u64, V> for Scalar {
    fn plan_key(
        _: &u64,
        _: &mut concread::bptree::AllocationDemand,
    ) -> Result<(), concread::bptree::PlanningError> {
        Ok(())
    }
    fn plan_value(
        _: &V,
        _: &mut concread::bptree::AllocationDemand,
    ) -> Result<(), concread::bptree::PlanningError> {
        Ok(())
    }
}
impl storage::AdmittedStoragePolicy for Scalar {
    fn from_admission(reservation: crate::allocation::AllocationReservation) -> Self {
        Self(reservation)
    }
    fn admission(&self) -> &crate::allocation::AllocationReservation {
        &self.0
    }
}

#[test]
fn detached_slots_prepaid_preserve_original_scope_and_refusal_custody_without_new_credits() {
    use crate::allocation::AllocationBudget;
    use concread::bptree::Prepaid;
    let budget = AllocationBudget::new(1 << 20);
    let target = Storage::<u64, u64, Prepaid<Scalar>>::try_new_admitted(budget.clone()).unwrap();
    let original = target
        .try_capture_admitted_block(BlockMode::Ordinary, |block| {
            block.try_insert_admitted(1, 44).unwrap();
            Ok::<_, ()>(())
        })
        .unwrap();
    let pointer = std::ptr::from_ref(original.touched_entries().next().unwrap().after.unwrap());
    let foreign = AllocationBudget::new(1 << 20);
    let original = foreign.with_deferred_refund_notifications(|scope| {
        match original.try_publication_slot(scope, &target) {
            Err((original, Refusal::Admission(storage::AdmittedStorageError::ScopeIdentity))) => {
                original
            }
            _ => panic!("foreign scope must refuse before native acquisition"),
        }
    });
    budget.with_deferred_refund_notifications(|scope| {
        let before = budget.reserved_bytes();
        // Real writer contention after undo acquisition; no speculative reserve.
        let held = target.blocks.try_acquire_writer().unwrap();
        let mut slot = original.try_publication_slot(scope, &target).ok().unwrap();
        let wait = target.revert_released.observe();
        let mut future = wait.wait_for_release();
        assert!(matches!(slot.try_prepare(), Err(Refusal::Busy(_))));
        assert_eq!(budget.reserved_bytes(), before);
        let original = slot.recover_original();
        assert_eq!(
            std::ptr::from_ref(original.touched_entries().next().unwrap().after.unwrap()),
            pointer
        );
        assert!(
            Pin::new(&mut future)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        drop(held);
        drop(slot);
        assert!(
            Pin::new(&mut future)
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_ready()
        );
        let mut slot = original.try_publication_slot(scope, &target).ok().unwrap();
        slot.try_prepare().unwrap();
        assert_eq!(
            budget.reserved_bytes(),
            before,
            "preparation copies and allocates no generation"
        );
        let published = slot.into_prepared().publish();
        assert_eq!(target.view().get(&1), Some(&44));
        drop(published);
    });
    drop(target);
    assert_eq!(budget.reserved_bytes(), 0);
}
