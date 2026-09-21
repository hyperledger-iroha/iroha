//! Original partial Cell acquisition releases all held writers before native wakes.

use super::*;
use crate::allocation::{AllocationBudget, AllocationCharge};
use std::{
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    },
    task::{Context, Wake, Waker},
};

#[derive(Default)]
struct CloneControl {
    fail_on: AtomicUsize,
    calls: [AtomicUsize; 2],
}

struct Payload {
    role: usize,
    bytes: Vec<u64>,
    control: Arc<CloneControl>,
}

impl Clone for Payload {
    fn clone(&self) -> Self {
        self.control.calls[self.role].fetch_add(1, SeqCst);
        assert!(
            self.control
                .fail_on
                .compare_exchange(self.role + 1, 0, SeqCst, SeqCst)
                .is_err(),
            "original payload clone panic"
        );
        Self {
            role: self.role,
            bytes: self.bytes.clone(),
            control: Arc::clone(&self.control),
        }
    }
}

fn seeded<C: Send + Sync + 'static>(
    charges: CellAllocationCharges<C>,
) -> (Arc<Cell<Payload, C>>, Arc<CloneControl>) {
    let control = Arc::new(CloneControl::default());
    let cell = Cell::from_values_charged(
        Payload {
            role: 0,
            bytes: vec![20, 21],
            control: Arc::clone(&control),
        },
        Some(Payload {
            role: 1,
            bytes: vec![10, 11],
            control: Arc::clone(&control),
        }),
        charges,
    );
    (Arc::new(cell), control)
}

#[derive(Default)]
struct Observed {
    calls: [AtomicUsize; 2],
    saw_held_writer: AtomicBool,
    poison_pairs: AtomicUsize,
}

struct Probe<C: Send + Sync + 'static> {
    cell: Arc<Cell<Payload, C>>,
    source: usize,
    observed: Arc<Observed>,
}

fn physically_released<T: Value, C: Send + Sync + 'static>(cell: &EbrCell<T, C>) -> bool {
    // Poison is set when the original mutex guard is released. For a healthy
    // writer use an actual nonblocking acquisition with a refused charge, so
    // this wake performs no payload clone or allocation and cannot deadlock.
    cell.is_poisoned() || matches!(cell.try_write_charged(|_, _| Err::<C, ()>(())), Err(()))
}

impl<C: Send + Sync + 'static> Wake for Probe<C> {
    fn wake(self: Arc<Self>) {
        self.observed.saw_held_writer.fetch_or(
            !physically_released(&self.cell.revert) || !physically_released(&self.cell.blocks),
            SeqCst,
        );
        let poison_pair = usize::from(self.cell.revert.is_poisoned()) * 2
            + usize::from(self.cell.blocks.is_poisoned());
        self.observed
            .poison_pairs
            .fetch_or(1 << poison_pair, SeqCst);
        self.observed.calls[self.source].fetch_add(1, SeqCst);
    }
}

fn arm<C: Send + Sync + 'static>(
    cell: &Arc<Cell<Payload, C>>,
) -> (
    Arc<Observed>,
    [concread::release::ReleaseFuture; 2],
    [Waker; 2],
) {
    let observed = Arc::new(Observed::default());
    let wakers = std::array::from_fn(|source| {
        Waker::from(Arc::new(Probe {
            cell: Arc::clone(cell),
            source,
            observed: Arc::clone(&observed),
        }))
    });
    let mut waits = [
        cell.revert_released.observe().wait_for_release(),
        cell.blocks_released.observe().wait_for_release(),
    ];
    for (wait, waker) in waits.iter_mut().zip(&wakers) {
        assert!(
            Pin::new(wait)
                .poll(&mut Context::from_waker(waker))
                .is_pending()
        );
    }
    (observed, waits, wakers)
}

fn assert_signals<C: Send + Sync + 'static>(
    cell: &Cell<Payload, C>,
    observed: &Observed,
    mut waits: [concread::release::ReleaseFuture; 2],
    wakers: &[Waker; 2],
    counts: [usize; 2],
    poison_pair: usize,
) {
    assert_eq!(
        observed.calls.each_ref().map(|count| count.load(SeqCst)),
        counts
    );
    assert!(!observed.saw_held_writer.load(SeqCst));
    assert_eq!(observed.poison_pairs.load(SeqCst), 1 << poison_pair);
    for ((wait, waker), count) in waits.iter_mut().zip(wakers).zip(counts) {
        assert_eq!(
            Pin::new(wait)
                .poll(&mut Context::from_waker(waker))
                .is_ready(),
            count != 0
        );
    }
    assert_eq!(
        cell.revert_released.observe().is_poisoned(),
        cell.revert.is_poisoned()
    );
    assert_eq!(
        cell.blocks_released.observe().is_poisoned(),
        cell.blocks.is_poisoned()
    );
}

fn pointers<C: Send + Sync + 'static>(cell: &Cell<Payload, C>) -> [*const u64; 2] {
    [
        cell.view().bytes.as_ptr(),
        cell.predecessor_view().as_ref().unwrap().bytes.as_ptr(),
    ]
}

fn assert_original<C: Send + Sync + 'static>(cell: &Cell<Payload, C>, original: [*const u64; 2]) {
    assert_eq!(pointers(cell), original);
    assert_eq!(cell.view().bytes, [20, 21]);
    assert_eq!(cell.predecessor_view().as_ref().unwrap().bytes, [10, 11]);
}

fn abandon<C: Send + Sync + 'static>(
    cell: &Cell<Payload, C>,
    charges: CellAllocationCharges<C>,
    mode: usize,
) {
    match mode {
        0 => drop(cell.block_charged(charges)),
        1 => drop(cell.block_and_revert_charged(charges)),
        _ => drop(cell.current_replacement_charged(charges)),
    }
}

#[test]
fn second_clone_unwind_releases_both_original_writers_before_either_wake() {
    for mode in 0..3 {
        let (cell, control) = seeded(CellAllocationCharges::untracked());
        let original = pointers(&cell);
        let predecessor = cell.publication.capture();
        let (observed, waits, wakers) = arm(&cell);
        control.fail_on.store(1, SeqCst);
        let error = catch_unwind(AssertUnwindSafe(|| match mode {
            0 => drop(cell.block()),
            1 => drop(cell.block_and_revert()),
            _ => drop(cell.current_replacement()),
        }))
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<&str>(),
            Some(&"original payload clone panic")
        );
        assert_eq!(
            control.calls.each_ref().map(|count| count.load(SeqCst)),
            [1, 1]
        );
        assert_signals(&cell, &observed, waits, &wakers, [1, 1], 3);
        assert_original(&cell, original);
        assert!(predecessor.matches(&cell.publication));
    }
}

fn charges(budget: &AllocationBudget) -> CellAllocationCharges<AllocationCharge> {
    let [current, undo] = Cell::<Payload, AllocationCharge>::allocation_layouts();
    let mut reserved = budget.try_reserve_layouts([current, undo]).unwrap();
    CellAllocationCharges::new(
        reserved.try_split(current).unwrap(),
        reserved.try_split(undo).unwrap(),
    )
}

#[test]
fn failed_second_clone_refunds_finished_undo_but_retains_its_own_charge() {
    for mode in 0..3 {
        let [current, undo] = Cell::<Payload, AllocationCharge>::allocation_layouts();
        let pair = current.size() + undo.size();
        let budget = AllocationBudget::new(2 * pair);
        let (cell, control) = seeded(charges(&budget));
        let original = pointers(&cell);
        let predecessor = cell.publication.capture();
        let (observed, waits, wakers) = arm(&cell);
        let prepaid = charges(&budget);
        assert_eq!(budget.reserved_bytes(), 2 * pair);
        control.fail_on.store(1, SeqCst);
        let error = catch_unwind(AssertUnwindSafe(|| abandon(&cell, prepaid, mode))).unwrap_err();
        assert_eq!(
            error.downcast_ref::<&str>(),
            Some(&"original payload clone panic")
        );
        assert_eq!(
            control.calls.each_ref().map(|count| count.load(SeqCst)),
            [1, 1]
        );
        assert_signals(&cell, &observed, waits, &wakers, [1, 1], 3);
        assert_eq!(budget.reserved_bytes(), pair + current.size());
        // Concread conservatively retains a failed Clone's already admitted
        // charge. Only the fully constructed undo clone is reclaimed/refunded;
        // these exact outer layouts do not fund this fixture's nested Vec.
        assert_original(&cell, original);
        assert!(predecessor.matches(&cell.publication));
    }
}

#[test]
fn already_poisoned_second_writer_refunds_unused_and_abandoned_charges() {
    for mode in 0..3 {
        let [current, undo] = Cell::<Payload, AllocationCharge>::allocation_layouts();
        let pair = current.size() + undo.size();
        let budget = AllocationBudget::new(2 * pair);
        let (cell, control) = seeded(charges(&budget));
        // Poison the actual current mutex without constructing a generation.
        let poison = catch_unwind(AssertUnwindSafe(|| {
            let _ = cell
                .blocks
                .write_charged(|_, _| -> Result<AllocationCharge, ()> {
                    panic!("original current admission panic")
                });
        }))
        .unwrap_err();
        assert_eq!(
            poison.downcast_ref::<&str>(),
            Some(&"original current admission panic")
        );
        assert!(cell.blocks.is_poisoned());
        let original = pointers(&cell);
        let predecessor = cell.publication.capture();
        let (observed, waits, wakers) = arm(&cell);
        assert!(catch_unwind(AssertUnwindSafe(|| abandon(&cell, charges(&budget), mode))).is_err());
        assert_eq!(
            control.calls.each_ref().map(|count| count.load(SeqCst)),
            [0, 0]
        );
        assert_signals(&cell, &observed, waits, &wakers, [1, 1], 3);
        assert_eq!(budget.reserved_bytes(), pair);
        assert_original(&cell, original);
        assert!(predecessor.matches(&cell.publication));
    }
}

#[test]
fn first_clone_unwind_releases_both_preacquired_writers() {
    let (cell, control) = seeded(CellAllocationCharges::untracked());
    let original = pointers(&cell);
    let predecessor = cell.publication.capture();
    let (observed, waits, wakers) = arm(&cell);
    control.fail_on.store(2, SeqCst);
    let error = catch_unwind(AssertUnwindSafe(|| drop(cell.block()))).unwrap_err();
    assert_eq!(
        error.downcast_ref::<&str>(),
        Some(&"original payload clone panic")
    );
    assert_eq!(
        control.calls.each_ref().map(|count| count.load(SeqCst)),
        [0, 1]
    );
    // Both physical writers are acquired before the first payload clone.
    assert_signals(&cell, &observed, waits, &wakers, [1, 1], 3);
    assert_original(&cell, original);
    assert!(predecessor.matches(&cell.publication));
}

#[test]
fn completed_pair_keeps_notifications_until_consumption() {
    let (cell, _) = seeded(CellAllocationCharges::untracked());
    let original = pointers(&cell);
    let predecessor = cell.publication.capture();
    let (observed, waits, wakers) = arm(&cell);
    let pair = cell.acquire_charged_writers(CellAllocationCharges::untracked());
    assert_eq!(
        observed.calls.each_ref().map(|count| count.load(SeqCst)),
        [0, 0]
    );
    drop(pair);
    assert_signals(&cell, &observed, waits, &wakers, [1, 1], 0);
    assert_original(&cell, original);
    assert!(predecessor.matches(&cell.publication));
}
