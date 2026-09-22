//! Attached publication preserves original authority and defers aggregate cleanup.

use crate::{
    BlockMode, BlockPublication, BlockRetirement,
    cell::{self, Cell},
    publication::NextPublication,
    storage::{self, Storage, StorageReadOnly},
};
use concread::release::{ReleaseFuture, ReleaseWait};
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
        map: Storage::new(),
    });
    let mut first = target.map.block();
    first.insert(1, 10);
    first.commit();
    let mut cell = target.cell.block();
    *cell.get_mut() = 20;
    cell.commit();
    let mut second = target.map.block();
    second.insert(1, 20);
    second.commit();
    target
}

struct Pair<'a> {
    cell: cell::BlockPublicationSlot<'a, u64>,
    map: storage::BlockPublicationSlot<'a, u64, u64>,
}
impl Drop for Pair<'_> {
    fn drop(&mut self) {
        self.cell.release_writers();
        self.map.release_writers();
    }
}

fn original_pair(target: &Targets, mode: BlockMode, edit: bool) -> Pair<'_> {
    let mut cell = match mode {
        BlockMode::Ordinary => target.cell.block(),
        BlockMode::Replace => target.cell.block_and_revert(),
    };
    let mut map = match mode {
        BlockMode::Ordinary => target.map.block(),
        BlockMode::Replace => target.map.block_and_revert(),
    };
    if edit {
        *cell.get_mut() = 33;
        map.insert(1, 33);
    }
    Pair {
        cell: cell.publication_slot(),
        map: map.publication_slot(),
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
        // These are actual raw acquisitions: no clone, cursor construction or
        // allocation. Some also retains a poisoned mutex; poison is not Busy.
        let cu = self.target.cell.revert.try_acquire_writer();
        let cc = self.target.cell.blocks.try_acquire_writer();
        let mu = self.target.map.revert.try_acquire_writer();
        let mc = self.target.map.blocks.try_acquire_writer();
        let held = usize::from(cu.is_none())
            + usize::from(cc.is_none())
            + usize::from(mu.is_none())
            + usize::from(mc.is_none());
        let poisoned = usize::from(cu.as_ref().is_some_and(|g| g.is_poisoned()))
            + usize::from(cc.as_ref().is_some_and(|g| g.is_poisoned()))
            + usize::from(mu.as_ref().is_some_and(|g| g.is_poisoned()))
            + usize::from(mc.as_ref().is_some_and(|g| g.is_poisoned()));
        self.held.fetch_add(held, SeqCst);
        self.poisoned.fetch_add(poisoned, SeqCst);
        self.calls.fetch_add(1, SeqCst);
        // No assertions, payloads, or MV notification guards exist in Wake.
    }
}

fn arm(target: &Arc<Targets>) -> (Arc<Probe>, ReleaseWait, ReleaseFuture, Waker) {
    let probe = Arc::new(Probe {
        target: Arc::clone(target),
        calls: AtomicUsize::new(0),
        held: AtomicUsize::new(0),
        poisoned: AtomicUsize::new(0),
    });
    let wait = target.cell.revert_released.observe();
    let mut future = wait.clone().wait_for_release();
    let waker = Waker::from(Arc::clone(&probe));
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    (probe, wait, future, waker)
}

fn assert_complete(probe: &Probe, mut future: ReleaseFuture, waker: &Waker) {
    assert!(
        Pin::new(&mut future)
            .poll(&mut Context::from_waker(waker))
            .is_ready()
    );
    assert_eq!(probe.calls.load(SeqCst), 1);
    assert_eq!(
        probe.held.load(SeqCst),
        0,
        "original callback saw a held physical sibling"
    );
}

#[test]
fn attached_publication_preserves_both_modes_and_retains_success_cleanup() {
    for mode in [BlockMode::Ordinary, BlockMode::Replace] {
        for edit in [false, true] {
            let target = seeded();
            let old_cell = target.cell.view();
            let old_map = target.map.view();
            let mut pair = original_pair(&target, mode, edit);
            let (probe, wait, future, waker) = arm(&target);
            pair.cell.prepare_publication();
            pair.map.prepare_publication();
            assert_eq!(probe.calls.load(SeqCst), 0);
            pair.cell.publish_prepared();
            assert_eq!(
                probe.calls.load(SeqCst),
                0,
                "first published field notified early"
            );
            pair.map.publish_prepared();
            assert_eq!(
                probe.calls.load(SeqCst),
                0,
                "published cleanup must remain caller-owned"
            );
            let before = if mode == BlockMode::Replace { 10 } else { 20 };
            let after = if edit { 33 } else { before };
            assert_eq!(*target.cell.view(), after);
            assert_eq!(target.map.view().get(&1), Some(&after));
            assert_eq!(*old_cell, 20);
            assert_eq!(old_map.get(&1), Some(&20));
            drop(pair);
            assert_complete(&probe, future, &waker);
            assert!(!wait.is_poisoned());
            assert_eq!(probe.poisoned.load(SeqCst), 0);
            let reverted_cell = target.cell.block_and_revert();
            let reverted_map = target.map.block_and_revert();
            assert_eq!(*reverted_cell.get(), before);
            assert_eq!(reverted_map.get(&1), Some(&before));
        }
    }
}

#[test]
fn attached_publication_abandonment_retains_every_prepared_original_until_joint_release() {
    let target = seeded();
    let mut pair = original_pair(&target, BlockMode::Ordinary, true);
    let (probe, wait, future, waker) = arm(&target);
    pair.cell.prepare_publication();
    pair.map.prepare_publication();
    pair.cell.release_writers();
    assert_eq!(probe.calls.load(SeqCst), 0);
    pair.map.release_writers();
    assert_eq!(probe.calls.load(SeqCst), 0);
    // Cleanup-only release never grants publication back, and rejection leaves
    // all retained payloads in their original caller slots until destruction.
    assert!(catch_unwind(AssertUnwindSafe(|| pair.cell.publish_prepared())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| pair.map.publish_prepared())).is_err());
    assert_eq!(probe.calls.load(SeqCst), 0);
    drop(pair);
    assert_complete(&probe, future, &waker);
    assert!(!wait.is_poisoned());
    assert_eq!(*target.cell.view(), 20);
    assert_eq!(target.map.view().get(&1), Some(&20));
}

#[test]
fn attached_publication_late_identity_poison_keeps_original_pair_and_rejects_retry() {
    let target = seeded();
    let mut pair = original_pair(&target, BlockMode::Ordinary, true);
    let (probe, wait, future, waker) = arm(&target);
    // Poison only the actual map identity lock, after original data acquisition.
    assert!(
        catch_unwind(AssertUnwindSafe(|| target
            .map
            .publication
            .publish_retaining(
                NextPublication::new(),
                || panic!("actual identity-lock unwind"),
                |_: ()| (),
            )))
        .is_err()
    );
    pair.cell.prepare_publication();
    assert!(catch_unwind(AssertUnwindSafe(|| pair.map.prepare_publication())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| pair.map.prepare_publication())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| pair.map.publish_prepared())).is_err());
    assert_eq!(probe.calls.load(SeqCst), 0);
    assert!(target.cell.blocks.try_acquire_writer().is_none());
    assert!(target.map.blocks.try_acquire_writer().is_none());
    drop(pair);
    assert_complete(&probe, future, &waker);
    assert!(
        !wait.is_poisoned(),
        "caught local failure did not unwind original data guards"
    );
    assert_eq!(probe.poisoned.load(SeqCst), 0);
    assert_eq!(*target.cell.view(), 20);
    assert_eq!(target.map.view().get(&1), Some(&20));
}

#[test]
fn attached_publication_unwind_releases_all_raw_owners_before_original_wake() {
    let target = seeded();
    let pair = original_pair(&target, BlockMode::Replace, true);
    let (probe, wait, future, waker) = arm(&target);
    assert!(
        catch_unwind(AssertUnwindSafe(move || {
            let mut pair = pair;
            pair.cell.prepare_publication();
            pair.map.prepare_publication();
            panic!("later aggregate preparation unwound");
        }))
        .is_err()
    );
    assert_complete(&probe, future, &waker);
    assert!(wait.is_poisoned());
    assert_eq!(probe.poisoned.load(SeqCst), 4);
    // Poisoned data locks are retained verdicts, not held locks or new journals.
}
