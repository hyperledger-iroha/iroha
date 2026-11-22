//! Original mixed Cell/map suspension keeps callbacks behind aggregate release.

use super::*;
use crate::{
    allocation_test_support::without_allocations,
    cell::Cell,
    storage::{Storage, StorageReadOnly as _},
};
use iroha_allocation::{
    AllocationBudget,
    release::{ReleaseFuture, ReleaseRegistration, ReleaseWait},
};
use std::{
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
    task::{Context, Wake, Waker},
};

struct Pair {
    cell: Cell<u64>,
    map: Storage<u64, u64>,
    fence: Mutex<()>,
}
impl Pair {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            cell: Cell::new(10),
            map: Storage::from_iter([(1, 10)]),
            fence: Mutex::new(()),
        })
    }
    fn any_held(&self) -> bool {
        [
            self.cell.revert.try_acquire_writer().is_none(),
            self.cell.blocks.try_acquire_writer().is_none(),
            self.map.revert.try_acquire_writer().is_none(),
            self.map.blocks.try_acquire_writer().is_none(),
            self.fence.try_lock().is_err(),
        ]
        .into_iter()
        .any(|held| held)
    }
    fn writers(&self) -> [ReleaseWait; 4] {
        [
            self.cell.revert_released.observe(),
            self.cell.blocks_released.observe(),
            self.map.revert_released.observe(),
            self.map.blocks_released.observe(),
        ]
    }
}
struct Probe {
    pair: Arc<Pair>,
    calls: AtomicUsize,
    held: AtomicUsize,
    panic_once: AtomicBool,
}
impl Probe {
    fn new(pair: &Arc<Pair>, panic_once: bool) -> Arc<Self> {
        Arc::new(Self {
            pair: Arc::clone(pair),
            calls: AtomicUsize::new(0),
            held: AtomicUsize::new(0),
            panic_once: AtomicBool::new(panic_once),
        })
    }
}
impl Wake for Probe {
    fn wake(self: Arc<Self>) {
        self.calls.fetch_add(1, SeqCst);
        if self.pair.any_held() {
            self.held.fetch_add(1, SeqCst);
        }
        assert!(
            !self.panic_once.swap(false, SeqCst),
            "original wake interruption"
        );
    }
}
fn arm<'a>(
    sources: impl IntoIterator<Item = ReleaseWait>,
    registrations: &'a mut [ReleaseRegistration],
    probe: &Arc<Probe>,
) -> Vec<ReleaseFuture<'a>> {
    let waker = Waker::from(Arc::clone(probe));
    sources
        .into_iter()
        .zip(registrations)
        .map(|(source, registration)| {
            let mut wait = source.wait_for_release(registration);
            assert!(
                Pin::new(&mut wait)
                    .poll(&mut Context::from_waker(&waker))
                    .is_pending()
            );
            wait
        })
        .collect()
}
fn assert_released(probe: &Arc<Probe>, waits: &mut [ReleaseFuture<'_>]) {
    assert_eq!(probe.calls.load(SeqCst), waits.len());
    assert_eq!(probe.held.load(SeqCst), 0);
    let waker = Waker::from(Arc::clone(probe));
    for wait in waits {
        assert!(
            Pin::new(wait)
                .poll(&mut Context::from_waker(&waker))
                .is_ready()
        );
    }
}

#[test]
fn suspension_notifies_only_after_every_original_writer_and_fence_releases() {
    let budget = AllocationBudget::new(4 * ReleaseRegistration::allocation_layout().size());
    let mut registrations: [_; 4] =
        std::array::from_fn(|_| crate::release_test_support::registration(&budget));
    for panic_wake in [false, true] {
        let pair = Pair::new();
        let mut cell = pair.cell.block();
        *cell.get_mut() = 20;
        let mut map = pair.map.block();
        map.insert(1, 20);
        let mut cell = cell.suspension_slot();
        let mut map = map.suspension_slot();
        let fence = pair.fence.lock().unwrap();
        let probe = Probe::new(&pair, panic_wake);
        let mut waits = arm(pair.writers(), &mut registrations, &probe);
        without_allocations(|| {
            cell.suspend();
            assert_eq!(probe.calls.load(SeqCst), 0);
            map.suspend();
        });
        let (cell, cell_cleanup) = cell.into_suspended();
        let (map, map_cleanup) = map.into_suspended();
        assert_eq!(probe.calls.load(SeqCst), 0);
        assert!(pair.any_held(), "the enclosing fence is still held");
        drop(fence);
        let release = || drop((cell_cleanup, map_cleanup));
        if panic_wake {
            assert!(catch_unwind(AssertUnwindSafe(release)).is_err());
        } else {
            without_allocations(release);
        }
        assert_released(&probe, &mut waits);
        assert_eq!(*pair.cell.view(), 10);
        assert_eq!(pair.map.view().get(&1), Some(&10));
        without_allocations(|| drop((cell, map)));
    }
}

#[test]
fn resumed_slots_retain_original_reader_notices_until_all_execution_writers_release() {
    let budget = AllocationBudget::new(6 * ReleaseRegistration::allocation_layout().size());
    let mut registrations: [_; 6] =
        std::array::from_fn(|_| crate::release_test_support::registration(&budget));
    let pair = Pair::new();
    let mut cell = pair.cell.block().suspension_slot();
    // Only a dirty current map prepares a reader retirement alongside its undo map.
    let mut map = pair.map.block();
    map.insert(1, 20);
    let mut map = map.suspension_slot();
    cell.suspend();
    map.suspend();
    let (cell, cell_cleanup) = cell.into_suspended();
    let (map, map_cleanup) = map.into_suspended();
    drop((cell_cleanup, map_cleanup));
    let mut cell_slot = cell.resumption_slot(&pair.cell);
    let mut map_slot = map.resumption_slot(&pair.map);
    let probe = Probe::new(&pair, false);
    let mut waits = arm(
        pair.writers().into_iter().chain([
            pair.map.blocks.observe_reader_release(),
            pair.map.revert.observe_reader_release(),
        ]),
        &mut registrations,
        &probe,
    );
    let fence = pair.fence.lock().unwrap();
    without_allocations(|| {
        cell_slot.try_resume().unwrap();
        map_slot.try_resume().unwrap();
    });
    assert_eq!(probe.calls.load(SeqCst), 0);
    let mut cell = cell_slot.take_block();
    let mut map = map_slot.take_block();
    without_allocations(|| {
        cell.release_writers();
        assert_eq!(
            probe.calls.load(SeqCst),
            0,
            "the map writers and fence remain held"
        );
        map.release_writers();
    });
    assert_eq!(probe.calls.load(SeqCst), 0);
    drop(fence);
    assert!(!pair.any_held());
    // Resumption moved actual writer guards back into the Blocks. Their release
    // notices stay in those Blocks, while the slots retain reader/identity notices.
    without_allocations(|| drop(cell));
    assert_eq!(probe.calls.load(SeqCst), 2);
    assert_eq!(probe.held.load(SeqCst), 0);
    without_allocations(|| drop(map));
    assert_eq!(
        probe.calls.load(SeqCst),
        4,
        "all four original writer notices retire only after their physical owners and fence"
    );
    assert_eq!(probe.held.load(SeqCst), 0);
    let waker = Waker::from(Arc::clone(&probe));
    for (index, wait) in waits.iter_mut().enumerate() {
        let ready = Pin::new(wait)
            .poll(&mut Context::from_waker(&waker))
            .is_ready();
        assert_eq!(
            ready,
            index < 4,
            "the slots still retain both original map reader notices"
        );
    }
    without_allocations(|| drop((cell_slot, map_slot)));
    assert_released(&probe, &mut waits);
}

#[test]
fn partially_resumed_mixed_pair_releases_every_original_before_recovery_wakes() {
    let budget = AllocationBudget::new(4 * ReleaseRegistration::allocation_layout().size());
    let mut registrations: [_; 4] =
        std::array::from_fn(|_| crate::release_test_support::registration(&budget));
    let pair = Pair::new();
    let mut cell = pair.cell.block().suspension_slot();
    let mut map = pair.map.block().suspension_slot();
    cell.suspend();
    map.suspend();
    let (cell, cell_cleanup) = cell.into_suspended();
    let (map, map_cleanup) = map.into_suspended();
    drop((cell_cleanup, map_cleanup));
    // This separate actual writer owns its own deferred release, too. Its release
    // cannot wake the observer while the enclosing retry attempt still holds siblings.
    let mut held_release = pair.map.blocks_released.deferred_batch();
    let held = pair
        .map
        .blocks_released
        .poisoning_guard(pair.map.blocks.write());
    let mut cell_slot = cell.resumption_slot(&pair.cell);
    let mut map_slot = map.resumption_slot(&pair.map);
    let probe = Probe::new(&pair, false);
    let mut waits = arm(pair.writers(), &mut registrations, &probe);
    let fence = pair.fence.lock().unwrap();
    let expected = pair.map.blocks_released.observe();
    without_allocations(|| {
        cell_slot.try_resume().unwrap();
        assert_eq!(
            map_slot.try_resume(),
            Err(PublicationPreparationError::Busy(expected))
        );
    });
    assert_eq!(probe.calls.load(SeqCst), 0);
    let (cell, map) =
        without_allocations(|| (cell_slot.recover_suspended(), map_slot.recover_suspended()));
    // Merely creating a batch cannot defer another owner's release. Transfer the
    // actual guard's notification into its own original batch while unlocking it.
    without_allocations(|| {
        assert!(held.try_release_into(&mut held_release, drop).is_ok());
    });
    assert_eq!(probe.calls.load(SeqCst), 0);
    drop(fence);
    without_allocations(|| drop((cell_slot, map_slot, held_release)));
    assert_released(&probe, &mut waits);
    let mut cell = cell.resumption_slot(&pair.cell);
    let mut map = map.resumption_slot(&pair.map);
    without_allocations(|| {
        cell.try_resume().unwrap();
        map.try_resume().unwrap();
        cell.release_writers();
        map.release_writers();
        drop((cell, map));
    });
}

#[derive(Debug, Default)]
struct CloneControl {
    armed: AtomicBool,
    calls: AtomicUsize,
}
#[derive(Debug)]
struct Payload {
    value: u64,
    control: Arc<CloneControl>,
}
impl Clone for Payload {
    fn clone(&self) -> Self {
        self.control.calls.fetch_add(1, SeqCst);
        assert!(!self.control.armed.load(SeqCst), "payload clone is refused");
        Self {
            value: self.value,
            control: Arc::clone(&self.control),
        }
    }
}

#[test]
fn suspension_and_both_resumption_outcomes_never_invoke_refusing_payload_clones() {
    let control = Arc::new(CloneControl::default());
    let payload = |value| Payload {
        value,
        control: Arc::clone(&control),
    };
    let cell = Cell::new(payload(10));
    let map = Storage::from_iter([(1_u64, payload(10))]);
    let mut cell_block = cell.block();
    cell_block.get_mut().value = 20;
    let mut map_block = map.block();
    map_block.insert(1, payload(20));
    let cell_pointer = std::ptr::from_ref(cell_block.get());
    let map_pointer = std::ptr::from_ref(map_block.get(&1).unwrap());
    control.armed.store(true, SeqCst);
    assert!(
        catch_unwind(AssertUnwindSafe(|| cell_block.get().clone())).is_err(),
        "the same original payload clone really refuses"
    );
    let before = control.calls.load(SeqCst);
    without_allocations(|| {
        let mut a = cell_block.suspension_slot();
        let mut b = map_block.suspension_slot();
        a.suspend();
        b.suspend();
        let (a, a_cleanup) = a.into_suspended();
        let (b, b_cleanup) = b.into_suspended();
        drop((a_cleanup, b_cleanup));
        let held = map.blocks.try_acquire_writer().unwrap();
        let mut a = a.resumption_slot(&cell);
        let mut b = b.resumption_slot(&map);
        a.try_resume().unwrap();
        assert!(matches!(
            b.try_resume(),
            Err(PublicationPreparationError::Busy(_))
        ));
        let (a_original, b_original) = (a.recover_suspended(), b.recover_suspended());
        drop((a, b, held));
        let mut a = a_original.resumption_slot(&cell);
        let mut b = b_original.resumption_slot(&map);
        a.try_resume().unwrap();
        b.try_resume().unwrap();
        let mut a_block = a.take_block();
        let mut b_block = b.take_block();
        assert_eq!(std::ptr::from_ref(a_block.get()), cell_pointer);
        assert_eq!(std::ptr::from_ref(b_block.get(&1).unwrap()), map_pointer);
        a_block.release_writers();
        b_block.release_writers();
        drop((a_block, b_block, a, b));
    });
    assert_eq!(control.calls.load(SeqCst), before);
    assert_eq!(cell.view().value, 10);
    assert_eq!(map.view().get(&1).unwrap().value, 10);
}
