//! Caller-owned map slots retain replacement-prefix cleanup until aggregate unlock.

use super::*;
use crate::{BlockAcquisition, BlockRetirement};
use std::{
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind},
    pin::Pin,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Wake, Waker},
};
struct Control {
    target: Mutex<Weak<Target>>,
    armed: AtomicBool,
    wakes: AtomicUsize,
    busy: AtomicUsize,
    copied_prefix_drops: AtomicUsize,
    prefix_drop_busy: AtomicUsize,
}
struct Payload {
    id: usize,
    copied: bool,
    control: Arc<Control>,
}
impl Clone for Payload {
    fn clone(&self) -> Self {
        assert!(
            !(self.id == 11 && self.control.armed.load(Ordering::SeqCst)),
            "second undo preimage clone"
        );
        Self {
            id: self.id,
            copied: true,
            control: Arc::clone(&self.control),
        }
    }
}
impl Drop for Payload {
    fn drop(&mut self) {
        if self.id == 10 && self.copied && self.control.armed.load(Ordering::SeqCst) {
            self.control
                .copied_prefix_drops
                .fetch_add(1, Ordering::SeqCst);
            if self.control.any_busy() {
                self.control.prefix_drop_busy.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}
struct Target {
    first: Storage<usize, Payload>,
    second: Storage<usize, Payload>,
}
impl Control {
    fn any_busy(&self) -> bool {
        let Ok(target) = self.target.try_lock() else {
            return true;
        };
        let Some(target) = target.upgrade() else {
            return true;
        };
        [
            target.first.revert.try_acquire_writer().is_none(),
            target.first.blocks.try_acquire_writer().is_none(),
            target.second.revert.try_acquire_writer().is_none(),
            target.second.blocks.try_acquire_writer().is_none(),
        ]
        .into_iter()
        .any(|busy| busy)
    }
}
impl Wake for Control {
    fn wake(self: Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
        if self.any_busy() {
            self.busy.fetch_add(1, Ordering::SeqCst);
        }
    }
}
fn fixture() -> (Arc<Target>, Arc<Control>) {
    let control = Arc::new(Control {
        target: Mutex::new(Weak::new()),
        armed: AtomicBool::new(false),
        wakes: AtomicUsize::new(0),
        busy: AtomicUsize::new(0),
        copied_prefix_drops: AtomicUsize::new(0),
        prefix_drop_busy: AtomicUsize::new(0),
    });
    let value = |id| Payload {
        id,
        copied: false,
        control: Arc::clone(&control),
    };
    let second = Storage::from_iter([(0, value(10)), (1, value(11))]);
    {
        let mut block = second.block();
        block.insert(0, value(20));
        block.insert(1, value(21));
        block.commit();
    }
    let target = Arc::new(Target {
        first: Storage::from_iter([(0, value(7))]),
        second,
    });
    *control.target.lock().unwrap() = Arc::downgrade(&target);
    (target, control)
}
struct Pending<'a> {
    first: BlockAcquisitionSlot<'a, usize, Payload>,
    second: BlockAcquisitionSlot<'a, usize, Payload>,
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        self.first.release();
        self.second.release();
    }
}
fn register(
    source: &ReleaseNotification,
    control: &Arc<Control>,
) -> concread::release::ReleaseFuture {
    let mut wait = source.observe().wait_for_release();
    let waker = Waker::from(Arc::clone(control));
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    wait
}

#[test]
fn caller_owned_storage_slots_retain_replacement_prefix_until_all_writers_release() {
    let (target, control) = fixture();
    let reader = target.second.snapshot();
    let mut pending = Pending {
        first: target.first.block_acquisition(),
        second: target.second.block_acquisition(),
    };
    pending.first.initialize(BlockMode::Ordinary);
    let first_wait = register(&target.first.revert_released, &control);
    let second_wait = register(&target.second.revert_released, &control);
    control.armed.store(true, Ordering::SeqCst);
    assert!(
        catch_unwind(AssertUnwindSafe(|| pending
            .second
            .initialize(BlockMode::Replace)))
        .is_err()
    );
    assert_eq!(control.wakes.load(Ordering::SeqCst), 0);
    assert_eq!(control.copied_prefix_drops.load(Ordering::SeqCst), 0);
    drop(pending);
    assert_eq!(control.wakes.load(Ordering::SeqCst), 2);
    assert_eq!(control.busy.load(Ordering::SeqCst), 0);
    assert_eq!(control.copied_prefix_drops.load(Ordering::SeqCst), 1);
    assert_eq!(control.prefix_drop_busy.load(Ordering::SeqCst), 0);
    assert!(!control.any_busy());
    control.armed.store(false, Ordering::SeqCst);
    drop((first_wait, second_wait));
    assert_eq!(target.first.view().get(&0).unwrap().id, 7);
    assert_eq!(target.second.view().get(&0).unwrap().id, 20);
    assert_eq!(target.second.view().get(&1).unwrap().id, 21);
    assert_eq!(reader.current().get(&0).unwrap().id, 20);
    assert_eq!(
        reader.revert_map().get(&0).unwrap().as_ref().unwrap().id,
        10
    );
}

#[test]
fn completed_storage_slots_release_physical_writers_before_retirement_and_refuse_reuse() {
    let target = Storage::from_iter([(0usize, 7usize)]);
    let mut slot = target.block_acquisition();
    slot.initialize(BlockMode::Ordinary);
    let mut block = slot.into_block();
    block.insert(1, 8);
    let mut wait = target.blocks_released.observe().wait_for_release();
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    block.release_writers();
    block.release_writers();
    assert!(target.blocks.try_acquire_writer().is_some());
    assert!(target.revert.try_acquire_writer().is_some());
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    assert!(catch_unwind(AssertUnwindSafe(|| block.get(&0))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| block.insert(2, 9))).is_err());
    drop(block);
    assert!(
        Pin::new(&mut wait)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_ready()
    );
    assert_eq!(
        target
            .view()
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect::<Vec<_>>(),
        [(0, 7)]
    );
}
