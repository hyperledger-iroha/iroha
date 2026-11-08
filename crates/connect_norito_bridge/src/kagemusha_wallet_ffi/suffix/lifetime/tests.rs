use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Barrier,
};

struct DropCounter {
    drops: Arc<AtomicUsize>,
    outside_lock: Option<Box<dyn Fn() + Send>>,
}
impl DropCounter {
    fn new(drops: &Arc<AtomicUsize>) -> Self {
        Self {
            drops: Arc::clone(drops),
            outside_lock: None,
        }
    }
}
impl Drop for DropCounter {
    fn drop(&mut self) {
        if let Some(check) = self.outside_lock.take() {
            check();
        }
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

fn status<T>(result: Result<T>, expected: i32) {
    match result {
        Ok(_) => panic!("expected a refusal"),
        Err(failure) => assert_eq!(failure.status, expected),
    }
}

#[test]
fn end_drops_each_unused_review_once_and_refuses_late_handoff() {
    let lifetime = Lifetime::<DropCounter>::default();
    let drops = Arc::new(AtomicUsize::new(0));
    let first = lifetime.review(DropCounter::new(&drops)).unwrap();
    let second = lifetime.review(DropCounter::new(&drops)).unwrap();
    let clone = first.lifetime();
    lifetime.end();
    assert!(lifetime.ended());
    assert!(first.ended());
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    clone.end();
    status(clone.require_live(), CLOSED);
    status(first.take(), CLOSED);
    status(second.take(), CLOSED);
    status(lifetime.review(DropCounter::new(&drops)), CLOSED);
    assert_eq!(drops.load(Ordering::SeqCst), 3);
}

#[test]
fn ending_one_origin_does_not_consume_another_origins_review() {
    let first = Lifetime::<DropCounter>::default();
    let second = Lifetime::<DropCounter>::default();
    let drops = Arc::new(AtomicUsize::new(0));
    let one = first.review(DropCounter::new(&drops)).unwrap();
    let two = second.review(DropCounter::new(&drops)).unwrap();
    first.end();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    status(one.take(), CLOSED);
    second.require_live().unwrap();
    let accepted = two.take().unwrap();
    second.end();
    // A taken review is now execution-owned; ending the exchange cannot drop it twice.
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    drop(accepted);
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

#[test]
fn released_review_slots_are_reclaimed_but_live_slots_remain_bounded() {
    let lifetime = Lifetime::<DropCounter>::default();
    let drops = Arc::new(AtomicUsize::new(0));
    let mut cells: Vec<_> = (0..MAX_REVIEWS)
        .map(|_| lifetime.review(DropCounter::new(&drops)).unwrap())
        .collect();
    status(lifetime.review(DropCounter::new(&drops)), RESOURCE);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    drop(cells.pop().unwrap());
    cells.push(lifetime.review(DropCounter::new(&drops)).unwrap());
    lifetime.end();
    assert_eq!(drops.load(Ordering::SeqCst), MAX_REVIEWS + 2);
    drop(cells);
    assert_eq!(drops.load(Ordering::SeqCst), MAX_REVIEWS + 2);
}

#[test]
fn review_destructor_is_never_called_under_lifetime_or_cell_lock() {
    let lifetime = Lifetime::<DropCounter>::default();
    let drops = Arc::new(AtomicUsize::new(0));
    let cell = lifetime.review(DropCounter::new(&drops)).unwrap();
    let state = Arc::downgrade(&lifetime.state);
    let weak_cell = Arc::downgrade(&cell.cell);
    cell.cell.lock().unwrap().as_mut().unwrap().outside_lock = Some(Box::new(move || {
        let state = state.upgrade().unwrap();
        assert!(state.try_lock().is_ok());
        let cell = weak_cell.upgrade().unwrap();
        assert!(cell.try_lock().is_ok());
    }));
    lifetime.end();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn an_end_racing_take_has_one_owner_and_one_drop() {
    for _ in 0..64 {
        let lifetime = Lifetime::<DropCounter>::default();
        let drops = Arc::new(AtomicUsize::new(0));
        let review = lifetime.review(DropCounter::new(&drops)).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let start = Arc::clone(&barrier);
        let end = lifetime.clone();
        let ending = std::thread::spawn(move || {
            start.wait();
            end.end();
        });
        barrier.wait();
        match review.take() {
            Ok(accepted) => drop(accepted),
            Err(failure) => assert_eq!(failure.status, CLOSED),
        }
        ending.join().unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        status(lifetime.require_live(), CLOSED);
    }
}

#[test]
fn poison_refuses_use_and_end_still_releases_retained_review() {
    let lifetime = Lifetime::<DropCounter>::default();
    let drops = Arc::new(AtomicUsize::new(0));
    let review = lifetime.review(DropCounter::new(&drops)).unwrap();
    let poisoned = lifetime.clone();
    assert!(std::thread::spawn(move || {
        let _guard = poisoned.state.lock().unwrap();
        panic!("poison lifecycle for cleanup test");
    })
    .join()
    .is_err());
    assert!(lifetime.ended());
    status(lifetime.require_live(), INTERNAL);
    lifetime.end();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    status(review.take(), INTERNAL);
}
